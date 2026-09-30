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
	inbound   chan *[]byte // от ОС → WireGuard шифрует и шлёт на сервер (копии в буферах пула)
	pool      sync.Pool    // буферы под копии входящих пакетов (см. deliverInbound)
	outbound  chan []byte // от сервера, расшифровано → в ОС
	events    chan tun.Event
	closed    chan struct{}
	closeOnce sync.Once
}

func newWGTun(mtu int) *wgTun {
	t := &wgTun{
		mtu:      mtu,
		inbound:  make(chan *[]byte, wgTunQueueSize),
		outbound: make(chan []byte, wgTunQueueSize),
		events:   make(chan tun.Event, 1),
		closed:   make(chan struct{}),
	}
	t.pool.New = func() any {
		buf := make([]byte, mtu)
		return &buf
	}
	t.events <- tun.EventUp
	return t
}

// deliverInbound — пакет от ОС. КОПИЯ обязательна: gomobile передаёт
// []byte ПО ССЫЛКЕ на память Swift (gobind: «byte slices are passed by
// reference»), живую только на время вызова, а WireGuard шифрует пакет
// позже, из очереди. Без копии на iPhone тоннель «подключался», но сети не
// было: Swift освобождал память пакетов (autoreleasepool в pumpInbound), к
// моменту шифрования они были перезаписаны, сервер отвечал лишь на малую
// часть (utun: 1534 пакета наружу, 96 обратно). Тест на Mac этого не ловил
// — там пакеты живут в памяти Go. Буферы из пула — без аллокации на пакет.
func (t *wgTun) deliverInbound(pkt []byte) error {
	buf := t.pool.Get().(*[]byte)
	if cap(*buf) < len(pkt) {
		grown := make([]byte, len(pkt))
		buf = &grown
	}
	*buf = (*buf)[:len(pkt)]
	copy(*buf, pkt)
	select {
	case t.inbound <- buf:
		return nil
	case <-t.closed:
		t.pool.Put(buf)
		return os.ErrClosed
	default:
		t.pool.Put(buf)
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
	case buf := <-t.inbound:
		sizes[0] = copy(bufs[0][offset:], *buf)
		t.pool.Put(buf)
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
