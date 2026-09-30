package netunnel

import (
	"os"
	"sync"

	"github.com/amnezia-vpn/amneziawg-go/v3/tun"
)

// wgTunQueueSize — очередь пакетов в каждую сторону; при переполнении
// входящие от ОС пакеты отбрасываются (как обычная потеря пакетов), а не
// блокируют цикл чтения packetFlow в Swift.
const wgTunQueueSize = 256

// wgTun — tun.Device для WireGuard без настоящего файлового дескриптора:
// пакеты приходят из Swift (packetFlow.readPackets → WritePacket) и уходят
// в Swift (ReadPacket → packetFlow.writePackets) через каналы — тот же
// подход, что virtualTun для hysteria2, только без gVisor: WireGuard сам
// работает на уровне IP-пакетов, TCP/UDP-стек ему не нужен.
type wgTun struct {
	mtu       int
	inbound   chan []byte // от ОС → WireGuard шифрует и шлёт на сервер
	outbound  chan []byte // от сервера, расшифровано → в ОС
	events    chan tun.Event
	closed    chan struct{}
	closeOnce sync.Once
}

func newWGTun(mtu int) *wgTun {
	t := &wgTun{
		mtu:      mtu,
		inbound:  make(chan []byte, wgTunQueueSize),
		outbound: make(chan []byte, wgTunQueueSize),
		events:   make(chan tun.Event, 1),
		closed:   make(chan struct{}),
	}
	t.events <- tun.EventUp
	return t
}

// deliverInbound — пакет от ОС. gomobile уже отдал нам собственную копию
// байтов, копировать повторно не нужно.
func (t *wgTun) deliverInbound(pkt []byte) error {
	select {
	case t.inbound <- pkt:
		return nil
	case <-t.closed:
		return os.ErrClosed
	default:
		return nil // очередь полна — пакет теряется, TCP перешлёт
	}
}

func (t *wgTun) takeOutbound() ([]byte, error) {
	select {
	case pkt := <-t.outbound:
		return pkt, nil
	case <-t.closed:
		return nil, os.ErrClosed
	}
}

func (t *wgTun) Read(bufs [][]byte, sizes []int, offset int) (int, error) {
	select {
	case pkt := <-t.inbound:
		sizes[0] = copy(bufs[0][offset:], pkt)
		return 1, nil
	case <-t.closed:
		return 0, os.ErrClosed
	}
}

func (t *wgTun) Write(bufs [][]byte, offset int) (int, error) {
	for i, buf := range bufs {
		// WireGuard переиспользует буферы после возврата — копия обязательна
		pkt := append([]byte(nil), buf[offset:]...)
		select {
		case t.outbound <- pkt:
		case <-t.closed:
			return i, os.ErrClosed
		}
	}
	return len(bufs), nil
}

func (t *wgTun) File() *os.File           { return nil }
func (t *wgTun) MTU() (int, error)        { return t.mtu, nil }
func (t *wgTun) Name() (string, error)    { return "vrox-wg", nil }
func (t *wgTun) Events() <-chan tun.Event { return t.events }
func (t *wgTun) BatchSize() int           { return 1 }

func (t *wgTun) Close() error {
	t.closeOnce.Do(func() {
		close(t.closed)
		close(t.events)
	})
	return nil
}
