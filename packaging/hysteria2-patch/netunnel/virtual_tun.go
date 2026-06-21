package netunnel

import (
	"errors"
	"io"

	"github.com/sagernet/sing/common/buf"
)

// virtualTun реализует tun.Tun (io.ReadWriter + N.VectorisedWriter +
// Close) без настоящего файлового дескриптора — в отличие от
// app/internal/tun/server.go (который вызывает tun.New() и получает
// реальное системное TUN-устройство), здесь пакеты ходят через каналы,
// которые наполняет/вычитывает TunnelHandle — а его методы вызываются
// из Swift через gomobile-границу (NEPacketTunnelFlow.readPackets даёт
// пакеты В тоннель, writePackets забирает пакеты ИЗ тоннеля).
//
// gVisor-стек (tun.NewSystem) не делает разницы между "настоящим" Tun
// и этим — он работает только через интерфейс Read/Write, понятия не
// имея, откуда берутся байты.
type virtualTun struct {
	inbound  chan []byte // от Swift → читает gVisor stack через Read()
	outbound chan []byte // от gVisor stack (Write()) → отдаём в Swift через ReadPacket()
	closed   chan struct{}
}

func newVirtualTun() *virtualTun {
	return &virtualTun{
		inbound:  make(chan []byte, 256),
		outbound: make(chan []byte, 256),
		closed:   make(chan struct{}),
	}
}

func (t *virtualTun) Read(p []byte) (int, error) {
	select {
	case pkt := <-t.inbound:
		n := copy(p, pkt)
		return n, nil
	case <-t.closed:
		return 0, io.EOF
	}
}

func (t *virtualTun) Write(p []byte) (int, error) {
	cp := append([]byte(nil), p...)
	select {
	case t.outbound <- cp:
		return len(p), nil
	case <-t.closed:
		return 0, errors.New("netunnel: tun closed")
	}
}

func (t *virtualTun) WriteVectorised(buffers []*buf.Buffer) error {
	for _, b := range buffers {
		if _, err := t.Write(b.Bytes()); err != nil {
			return err
		}
	}
	return nil
}

func (t *virtualTun) Close() error {
	select {
	case <-t.closed:
		// уже закрыт — Close() может быть вызван и явно через
		// TunnelHandle.Stop(), и стеком при его собственном завершении
	default:
		close(t.closed)
	}
	return nil
}

// deliverInbound вызывается из TunnelHandle.WritePacket (т.е. из Swift):
// пакет от NEPacketTunnelFlow попадает в gVisor stack как будто пришёл
// из настоящего TUN-устройства.
func (t *virtualTun) deliverInbound(pkt []byte) error {
	cp := append([]byte(nil), pkt...)
	select {
	case t.inbound <- cp:
		return nil
	case <-t.closed:
		return errors.New("netunnel: tun closed")
	}
}

// takeOutbound вызывается из TunnelHandle.ReadPacket (т.е. из Swift, в
// цикле packetFlow.writePackets) — блокируется до следующего пакета,
// который gVisor stack хочет отправить наружу, либо до Stop().
func (t *virtualTun) takeOutbound() ([]byte, error) {
	select {
	case pkt := <-t.outbound:
		return pkt, nil
	case <-t.closed:
		return nil, io.EOF
	}
}
