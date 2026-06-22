package netunnel

import (
	"io"
	"net"
	"strconv"
	"time"

	"gvisor.dev/gvisor/pkg/tcpip/adapters/gonet"
	"gvisor.dev/gvisor/pkg/tcpip/transport/tcp"
	"gvisor.dev/gvisor/pkg/tcpip/transport/udp"
	"gvisor.dev/gvisor/pkg/waiter"

	"github.com/apernet/hysteria/core/v2/client"
)

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

		conn := gonet.NewTCPConn(&wq, ep)
		go relayTCP(conn, hyConn)
	}
}

func relayTCP(local net.Conn, remote io.ReadWriteCloser) {
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

		go relayUDP(local, rc, reqAddr)
		return true
	}
}

func relayUDP(local net.Conn, remote client.HyUDPConn, reqAddr string) {
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
