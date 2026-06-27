package netunnel

import (
	"io"
	"net"
	"strconv"
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
		go relayTCP(conn, hyConn)
	}
}

func relayTCP(local net.Conn, remote io.ReadWriteCloser) {
	defer activeRelays.Add(-1)
	defer local.Close()
	defer remote.Close()

	copyErrChan := make(chan error, 2)
	go func() {
		_, copyErr := io.Copy(remote, local)
		copyErrChan <- copyErr
	}()
	go func() {
		_, copyErr := io.Copy(local, remote)
		copyErrChan <- copyErr
	}()
	<-copyErrChan
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
		go relayUDP(local, rc, reqAddr)
		return true
	}
}

func relayUDP(local net.Conn, remote client.HyUDPConn, reqAddr string) {
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
