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

// maxActiveRelays — потолок одновременных relay-пар (TCP+UDP вместе,
// общий бюджет). Пойман вживую: один реальный сайт + Speedtest
// одновременно открывают десятки параллельных TCP/QUIC-потоков
// (HTML/CSS/JS/картинки/аналитика/шрифты с разных доменов) — каждый
// относительно небольшой буфер, умноженный на 50-80 одновременных
// соединений, и набирает RSS far за бюджет iOS NE (~50МБ, см. init() в
// netunnel.go), даже после того как сами QUIC receive windows уже
// занижены. Это оказалось ГЛАВНЫМ потребителем памяти под нагрузкой —
// занижение одних только окон на поток (без лимита на их количество)
// само по себе только ухудшило ситуацию в живом тесте. Сверх лимита —
// TCP получает RST (клиент сам переподключится/отвалится на это
// конкретное соединение, остальной браузинг не страдает), UDP-датаграмма
// тихо дропается (как обычный packet loss, нормально для UDP).
//
// 48 — отправная точка, не результат профилирования: возможны заметные
// stalls при реальном использовании (если лимит слишком тесный) — в
// этом случае стоит поднять до 64+ и заново замерить память.
const maxActiveRelays = 48

var activeRelays atomic.Int32

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
// чем её закрыть. Раньше (sing-tun's System stack) это делал встроенный
// udpnat.New(udpTimeout, ...) — после перехода на gVisor напрямую (см.
// netunnel.go) эта логика пропала, и без неё каждый уникальный UDP-поток
// (а DNS-запросы создают их пачками) держал две горутины + gVisor-
// endpoint НАВСЕГДА, до полной остановки тоннеля — реальная утечка при
// долгой сессии. 60с — то же значение, что используется как разумный
// дефолт NAT-таймаута для UDP в большинстве реализаций (включая
// исходный sidecar-путь).
const udpIdleTimeout = 60 * time.Second

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

func tcpForwarderHandler(hyClient client.Client) func(*tcp.ForwarderRequest) {
	return func(r *tcp.ForwarderRequest) {
		// Лимит ДО дозвона — нет смысла тратить TCP/QUIC-handshake на
		// соединение, которое всё равно зарежем сразу после.
		if activeRelays.Load() >= maxActiveRelays {
			r.Complete(true) // RST — браузер сам ретраит/откладывает, остальной браузинг не страдает
			return
		}

		id := r.ID()
		reqAddr := net.JoinHostPort(id.LocalAddress.String(), strconv.Itoa(int(id.LocalPort)))

		hyConn, err := hyClient.TCP(reqAddr)
		if err != nil {
			r.Complete(true) // RST — как и в server.go, ошибка не всплывает наверх
			return
		}

		var wq waiter.Queue
		ep, tcpErr := r.CreateEndpoint(&wq)
		if tcpErr != nil {
			r.Complete(true)
			_ = hyConn.Close()
			return
		}
		r.Complete(false)

		activeRelays.Add(1)
		conn := gonet.NewTCPConn(&wq, ep)
		connID := registerConn(func() { _ = conn.Close() })
		go relayTCP(connID, conn, hyConn)
	}
}

// tcpIdleTimeout — то же самое, что udpIdleTimeout, но для TCP, у
// которого раньше idle-таймаута не было ВООБЩЕ. Пойман вживую: память
// расширения переставала падать обратно после реального браузинга — не
// потому, что Go держит мусор (FreeOSMemory тикер на это уже отвечает,
// см. netunnel.go), а потому, что браузер сам держит HTTP keep-alive
// TCP-соединения открытыми ПОСЛЕ загрузки страницы, без передачи
// данных — это легитимное, живое (не мусорное) соединение в нашем
// реестре, GC его не освободит, потому что оно реально используется
// (просто не активно прямо сейчас). io.Copy сам по себе не даёт зацепки
// для отслеживания активности — заменён на ручной цикл с таймером.
const tcpIdleTimeout = 60 * time.Second

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
	defer activeRelays.Add(-1)
	defer local.Close()
	defer remote.Close()

	var lastActivity atomic.Int64
	lastActivity.Store(time.Now().UnixNano())

	copyErrChan := make(chan error, 2)
	go func() {
		_, copyErr := io.Copy(remote, &activityReader{r: local, last: &lastActivity})
		copyErrChan <- copyErr
	}()
	go func() {
		_, copyErr := io.Copy(local, &activityReader{r: remote, last: &lastActivity})
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

func udpForwarderHandler(hyClient client.Client) func(*udp.ForwarderRequest) bool {
	return func(r *udp.ForwarderRequest) bool {
		// Сверх лимита — тихо дропаем датаграмму (как обычный packet
		// loss, нормальное поведение для UDP, отправитель сам ретраит
		// при необходимости на уровне своего протокола).
		if activeRelays.Load() >= maxActiveRelays {
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

		rc, err := hyClient.UDP()
		if err != nil {
			_ = local.Close()
			return false
		}

		activeRelays.Add(1)
		connID := registerConn(func() { _ = local.Close() })
		go relayUDP(connID, local, rc, reqAddr)
		return true
	}
}

func relayUDP(connID uint64, local net.Conn, remote client.HyUDPConn, reqAddr string) {
	defer unregisterConn(connID)
	defer activeRelays.Add(-1)
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
		buf := make([]byte, 65535)
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
