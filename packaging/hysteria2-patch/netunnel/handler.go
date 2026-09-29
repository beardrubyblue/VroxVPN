package netunnel

import (
	"io"
	"net"
	"strconv"
	"sync"
	"sync/atomic"
	"time"

	"gvisor.dev/gvisor/pkg/tcpip/adapters/gonet"
	"gvisor.dev/gvisor/pkg/tcpip/transport/tcp"
	"gvisor.dev/gvisor/pkg/tcpip/transport/udp"
	"gvisor.dev/gvisor/pkg/waiter"

	"github.com/apernet/hysteria/core/v2/client"
)

// copyBufPool — пул буферов для io.CopyBuffer. 4 КиБ вместо дефолтных
// 32 КиБ io.Copy: при 64 TCP-relay (2 копии на каждый) экономия
// (32−4)×2×64 = 3.5 МиБ живой памяти. На throughput не влияет — данные
// всё равно уходят в QUIC-stream, который сам буферизует.
var copyBufPool = sync.Pool{
	New: func() any {
		buf := make([]byte, 4<<10)
		return &buf
	},
}

// udpBufPool — пул буферов для чтения UDP-датаграмм из gVisor-endpoint.
// Без пула каждый relayUDP аллоцировал 65535 байт навсегда (до конца
// жизни горутины) — при 32 relay это 2 МиБ, которые не возвращались в
// кучу, пока relay не закроется.
var udpBufPool = sync.Pool{
	New: func() any {
		buf := make([]byte, 65535)
		return &buf
	},
}

// maxTCPRelays/maxUDPRelays — раздельные потолки, настраиваемые из UI
// через applyRelayLimits (см. netunnel.go::Config). Дефолты 64/32 —
// агрессивно низкие для iOS NE (~50 МиБ jetsam): каждый TCP relay
// стоит ~50–100 КиБ живой памяти (gVisor-буфера + goroutine stacks +
// io.CopyBuffer). Сверх лимита — TCP получает RST (клиент сам
// переподключится/отвалится на это конкретное соединение, остальной
// браузинг не страдает), UDP-датаграмма тихо дропается (как обычный
// packet loss, нормально для UDP).
const (
	defaultMaxTCPRelays = 64
	defaultMaxUDPRelays = 32
	defaultIdleTimeout  = 30 * time.Second
)

// Лимиты без jetsam (macOS и iOS-сборка на Mac, см.
// Config.MemoryConstrained): 300с — дефолт Happ, долгие соединения не
// рвутся; потолки с запасом на браузер с десятками вкладок и на DNS
// (каждый запрос — отдельный UDP-поток до idle timeout).
const (
	unconstrainedMaxRelays   = 2048
	unconstrainedIdleTimeout = 300 * time.Second
)

var (
	maxTCPRelays = int32(defaultMaxTCPRelays)
	maxUDPRelays = int32(defaultMaxUDPRelays)

	activeTCPRelays atomic.Int32
	activeUDPRelays atomic.Int32
)

// applyRelayLimits переносит настраиваемые из UI лимиты (см.
// netunnel.go::Config) в переменные, которые реально читают
// tcpForwarderHandler/udpForwarderHandler/relayTCP/relayUDP. Нулевые
// значения в JSON (поле не задано на Rust-стороне) — фоллбек на дефолт,
// а не "лимит = 0" (что заблокировало бы вообще все соединения).
func applyRelayLimits(cfg *Config) {
	if !cfg.MemoryConstrained {
		maxTCPRelays, maxUDPRelays = unconstrainedMaxRelays, unconstrainedMaxRelays
		tcpIdleTimeout, udpIdleTimeout = unconstrainedIdleTimeout, unconstrainedIdleTimeout
		return
	}
	maxTCPRelays, maxUDPRelays = defaultMaxTCPRelays, defaultMaxUDPRelays
	tcpIdleTimeout, udpIdleTimeout = defaultIdleTimeout, defaultIdleTimeout
	if cfg.MaxTCPConnections > 0 {
		maxTCPRelays = int32(cfg.MaxTCPConnections)
	}
	if cfg.MaxUDPConnections > 0 {
		maxUDPRelays = int32(cfg.MaxUDPConnections)
	}
	if cfg.IdleTimeoutSeconds > 0 {
		idleTimeout := time.Duration(cfg.IdleTimeoutSeconds) * time.Second
		tcpIdleTimeout = idleTimeout
		udpIdleTimeout = idleTimeout
	}
}

// Реестр активных relay-соединений с временем создания — нужен, чтобы
// под реальным давлением памяти (см. evictOldestConn,
// netunnel.go::evictUnderMemoryPressurePeriodically) закрывать САМЫЕ
// СТАРЫЕ соединения, а не просто ждать, пока iOS убьёт всё расширение
// разом. До этого maxActiveRelays выше только не пускал НОВЫЕ
// соединения сверх потолка — старые при этом копились без разбора, raз
// открытые могли жить хоть до конца сессии. closeFn просто закрывает
// сторону(ы) net.Conn — существующие defer'ы в relayTCP/relayUDP сами
// разберут оставшуюся теardown-логику (ровно как при обычной ошибке
// чтения/записи), отдельного пути остановки не нужно.
type registeredConn struct {
	createdAt time.Time
	close     func()
}

var (
	connRegistryMu sync.Mutex
	connRegistry   = make(map[uint64]*registeredConn)
	nextConnID     uint64
)

func registerConn(closeFn func()) uint64 {
	connRegistryMu.Lock()
	defer connRegistryMu.Unlock()
	nextConnID++
	id := nextConnID
	connRegistry[id] = &registeredConn{createdAt: time.Now(), close: closeFn}
	return id
}

func unregisterConn(id uint64) {
	connRegistryMu.Lock()
	defer connRegistryMu.Unlock()
	delete(connRegistry, id)
}

// evictOldestConn закрывает самое долгоживущее активное соединение.
// Возвращает false, если эвиктить уже больше нечего.
func evictOldestConn() bool {
	connRegistryMu.Lock()
	var oldestID uint64
	var oldestTime time.Time
	found := false
	for id, c := range connRegistry {
		if !found || c.createdAt.Before(oldestTime) {
			oldestID, oldestTime, found = id, c.createdAt, true
		}
	}
	var closeFn func()
	if found {
		closeFn = connRegistry[oldestID].close
		delete(connRegistry, oldestID)
	}
	connRegistryMu.Unlock()
	if closeFn == nil {
		return false
	}
	closeFn()
	return true
}

// udpIdleTimeout — сколько ждать следующий пакет в UDP-"сессии" прежде
// чем её закрыть. 30с — агрессивно для iOS NE: DNS-запросы fire-and-
// forget (ответ за <1с), другие UDP-потоки (QUIC-inside-tunnel) имеют
// свой keepalive. Было 300с (дефолт Happ), что копило десятки мёртвых
// UDP-сессий при долгом браузинге. Настраивается из UI (applyRelayLimits).
var udpIdleTimeout = defaultIdleTimeout

// tcpForwarderHandler и udpForwarderHandler — обработчики для
// tcp.Forwarder/udp.Forwarder gVisor-стека (см. netunnel.go::StartTunnel,
// где они регистрируются через s.SetTransportProtocolHandler). Логика
// relay (дозвон через HyClient.TCP/UDP + io.Copy) — та же идея, что в
// app/internal/tun/server.go (sidecar-путь, tunHandler.NewConnection/
// NewPacketConnection), но точка входа другая: там — sing-tun's
// tun.Handler, здесь — gVisor's forwarder request.
//
// `stack.TransportEndpointID.LocalAddress`/`LocalPort` — это адрес
// НАЗНАЧЕНИЯ исходного пакета (с точки зрения стека, принимающего
// входящий SYN/датаграмму, "локальный" — это он сам, то есть та сторона,
// которую приложение пыталось достичь); `RemoteAddress`/`RemotePort` —
// отправитель внутри тоннеля. Дозваниваемся по Local*, а не Remote*.
//
// directDomains (обход VPN по доменам через DNS-сниффинг) здесь
// СОЗНАТЕЛЬНО не перенесён — по плану (docs/ARCHITECTURE.md, раздел
// macOS/NetworkExtension, Фаза 3) на NE-пути это становится статическим
// excludedRoutes на стороне Rust (config_gen.rs), а не runtime-сниффингом.

func tcpForwarderHandler(h *TunnelHandle) func(*tcp.ForwarderRequest) {
	return func(r *tcp.ForwarderRequest) {
		if activeTCPRelays.Load() >= maxTCPRelays {
			r.Complete(true)
			return
		}

		id := r.ID()
		reqAddr := net.JoinHostPort(id.LocalAddress.String(), strconv.Itoa(int(id.LocalPort)))

		hyConn, err := h.getClient().TCP(reqAddr)
		if err != nil {
			h.noteDialResult(false) // мёртвый тоннель? копим streak, форсим реконнект
			r.Complete(true)
			return
		}
		h.noteDialResult(true)

		var wq waiter.Queue
		ep, tcpErr := r.CreateEndpoint(&wq)
		if tcpErr != nil {
			r.Complete(true)
			_ = hyConn.Close()
			return
		}
		r.Complete(false)

		activeTCPRelays.Add(1)
		conn := gonet.NewTCPConn(&wq, ep)
		connID := registerConn(func() { _ = conn.Close() })
		go relayTCP(connID, conn, hyConn)
	}
}

// tcpIdleTimeout — главный рычаг против накопления памяти при листании
// Reels: браузер держит HTTP keep-alive TCP-соединения от КАЖДОГО видео
// открытыми после загрузки — это живые (не мусорные) соединения, GC их
// не освободит. При 300с таймауте (старый дефолт) за 5 минут листания
// копилось 50-150 соединений × ~100 КиБ = 5-15 МиБ только в relay-
// overhead. 30с — соединения от видео 30-секундной давности уже мертвы,
// базовый RSS остаётся низким. Настраивается из UI (applyRelayLimits).
var tcpIdleTimeout = defaultIdleTimeout

// activityReader оборачивает io.Reader и отмечает время последнего
// успешного чтения — используется для отслеживания активности в обе
// стороны (от браузера и от сервера), io.Copy сам такой хук не даёт.
type activityReader struct {
	r    io.Reader
	last *atomic.Int64 // unix-нано последней активности
}

func (a *activityReader) Read(p []byte) (int, error) {
	n, err := a.r.Read(p)
	if n > 0 {
		a.last.Store(time.Now().UnixNano())
	}
	return n, err
}

func relayTCP(connID uint64, local net.Conn, remote io.ReadWriteCloser) {
	defer unregisterConn(connID)
	defer activeTCPRelays.Add(-1)
	defer local.Close()
	defer remote.Close()

	var lastActivity atomic.Int64
	lastActivity.Store(time.Now().UnixNano())

	copyErrChan := make(chan error, 2)
	go func() {
		bp := copyBufPool.Get().(*[]byte)
		_, copyErr := io.CopyBuffer(remote, &activityReader{r: local, last: &lastActivity}, *bp)
		copyBufPool.Put(bp)
		copyErrChan <- copyErr
	}()
	go func() {
		bp := copyBufPool.Get().(*[]byte)
		_, copyErr := io.CopyBuffer(local, &activityReader{r: remote, last: &lastActivity}, *bp)
		copyBufPool.Put(bp)
		copyErrChan <- copyErr
	}()

	idleTicker := time.NewTicker(10 * time.Second)
	defer idleTicker.Stop()
	for {
		select {
		case <-copyErrChan:
			return
		case <-idleTicker.C:
			if time.Since(time.Unix(0, lastActivity.Load())) > tcpIdleTimeout {
				// Закрываем local — оба io.Copy получат ошибку чтения/
				// записи, цикл выйдет на следующей итерации через
				// copyErrChan, остальную teardown-логику доделают defer'ы.
				_ = local.Close()
			}
		}
	}
}

func udpForwarderHandler(h *TunnelHandle) func(*udp.ForwarderRequest) bool {
	return func(r *udp.ForwarderRequest) bool {
		if activeUDPRelays.Load() >= maxUDPRelays {
			return false
		}

		id := r.ID()
		reqAddr := net.JoinHostPort(id.LocalAddress.String(), strconv.Itoa(int(id.LocalPort)))

		var wq waiter.Queue
		ep, tcpErr := r.CreateEndpoint(&wq)
		if tcpErr != nil {
			return false
		}
		local := gonet.NewUDPConn(&wq, ep)

		rc, err := h.getClient().UDP()
		if err != nil {
			h.noteDialResult(false)
			_ = local.Close()
			return false
		}
		h.noteDialResult(true)

		activeUDPRelays.Add(1)
		connID := registerConn(func() { _ = local.Close() })
		go relayUDP(connID, local, rc, reqAddr)
		return true
	}
}

func relayUDP(connID uint64, local net.Conn, remote client.HyUDPConn, reqAddr string) {
	defer unregisterConn(connID)
	defer activeUDPRelays.Add(-1)
	defer local.Close()
	defer remote.Close()

	copyErrChan := make(chan error, 2)
	// local -> remote: всё, что приходит из тоннеля на этот UDP-эндпоинт,
	// уходит на ОДИН адрес назначения (reqAddr) — gVisor UDP forwarder
	// создаёт отдельный endpoint на каждый уникальный (src, dst), так что
	// здесь нет смешивания разных направлений, как было бы в общем сокете.
	//
	// SetReadDeadline сбрасывается на каждый успешный пакет — без этого
	// сессия (и обе горутины) висела бы до закрытия всего тоннеля, даже
	// если по этому UDP-потоку больше никогда ничего не придёт (см.
	// udpIdleTimeout выше).
	go func() {
		bp := udpBufPool.Get().(*[]byte)
		defer udpBufPool.Put(bp)
		buf := *bp
		for {
			_ = local.SetReadDeadline(time.Now().Add(udpIdleTimeout))
			n, err := local.Read(buf)
			if err != nil {
				copyErrChan <- err
				return
			}
			if sendErr := remote.Send(append([]byte(nil), buf[:n]...), reqAddr); sendErr != nil {
				copyErrChan <- sendErr
				return
			}
		}
	}()
	// remote -> local. client.HyUDPConn (HyClient.UDP()) не поддерживает
	// SetReadDeadline (это интерфейс к hysteria2-сессии, не net.Conn) —
	// для этого направления идle-таймаут срабатывает косвенно: когда
	// горутина выше получит таймаут на local.Read(), relayUDP вернётся
	// из `<-copyErrChan` и выполнит defer'ы (local.Close()/remote.
	// Close()) — это разблокирует remote.Receive() здесь с ошибкой.
	go func() {
		for {
			bs, _, err := remote.Receive()
			if err != nil {
				copyErrChan <- err
				return
			}
			if _, writeErr := local.Write(bs); writeErr != nil {
				copyErrChan <- writeErr
				return
			}
		}
	}()
	<-copyErrChan
}
