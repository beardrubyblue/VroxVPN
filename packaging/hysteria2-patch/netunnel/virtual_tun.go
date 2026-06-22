package netunnel

import (
	"context"
	"errors"
	"io"

	"gvisor.dev/gvisor/pkg/buffer"
	"gvisor.dev/gvisor/pkg/tcpip"
	"gvisor.dev/gvisor/pkg/tcpip/header"
	"gvisor.dev/gvisor/pkg/tcpip/link/channel"
	"gvisor.dev/gvisor/pkg/tcpip/stack"
)

// virtualTun — точка-точка LinkEndpoint для gVisor netstack (через
// gvisor.dev/gvisor/pkg/tcpip/link/channel), без настоящего файлового
// дескриптора. Пакеты ходят через каналы channel.Endpoint, которые
// наполняет/вычитывает TunnelHandle — а его методы вызываются из Swift
// через gomobile-границу (NEPacketTunnelFlow.readPackets даёт пакеты В
// тоннель, writePackets забирает пакеты ИЗ тоннеля).
//
// ⚠ ИСТОРИЯ: до этого здесь была обёртка над `tun.NewSystem` из
// apernet/sing-tun, в комментариях ошибочно названная "gVisor netstack".
// Проверка на реальном Mac показала: tun.NewSystem — это "System
// stack", который требует НАСТОЯЩЕГО TUN-устройства (открывает обычный
// TCP-listener ОС, забинденный на IP виртуальной подсети — без
// настоящего TUN-интерфейса с этим IP в системе бинд падает с "can't
// assign requested address", железно, не временная ошибка). Сам gVisor
// в форке apernet/sing-tun вырезан целиком (см. stack_gvisor_stub.go в
// этом форке — заглушка с ошибкой "gVisor is not supported in this
// fork"), поэтому переключиться на `tun.NewGVisor` было нельзя.
// Решение: подключить `gvisor.dev/gvisor` напрямую, без обёртки
// sing-tun — `channel.Endpoint` именно для этого и существует (инъекция
// пакетов программно, без реального устройства), это тот же подход,
// что использует wireguard-go в своём netstack-режиме.
type virtualTun struct {
	ep     *channel.Endpoint
	ctx    context.Context
	cancel context.CancelFunc
}

func newVirtualTun(mtu uint32) *virtualTun {
	ctx, cancel := context.WithCancel(context.Background())
	return &virtualTun{
		ep:     channel.New(256, mtu, ""),
		ctx:    ctx,
		cancel: cancel,
	}
}

// deliverInbound вызывается из TunnelHandle.WritePacket (т.е. из Swift):
// пакет от NEPacketTunnelFlow попадает в gVisor stack как будто пришёл
// из настоящего TUN-устройства. Версия IP (4 или 6) определяется по
// старшему ниблу первого байта — так же, как и на стороне Swift при
// определении protocol number для packetFlow.writePackets.
func (t *virtualTun) deliverInbound(pkt []byte) error {
	if len(pkt) == 0 {
		return errors.New("netunnel: empty packet")
	}
	var proto tcpip.NetworkProtocolNumber
	switch pkt[0] >> 4 {
	case 4:
		proto = header.IPv4ProtocolNumber
	case 6:
		proto = header.IPv6ProtocolNumber
	default:
		return errors.New("netunnel: unknown IP version in packet")
	}

	cp := append([]byte(nil), pkt...)
	pb := stack.NewPacketBuffer(stack.PacketBufferOptions{
		Payload: buffer.MakeWithData(cp),
	})
	defer pb.DecRef()
	t.ep.InjectInbound(proto, pb)
	return nil
}

// takeOutbound вызывается из TunnelHandle.ReadPacket (т.е. из Swift, в
// цикле packetFlow.writePackets) — блокируется до следующего пакета,
// который gVisor stack хочет отправить наружу, либо до Close().
func (t *virtualTun) takeOutbound() ([]byte, error) {
	pb := t.ep.ReadContext(t.ctx)
	if pb == nil {
		return nil, io.EOF
	}
	defer pb.DecRef()
	buf := pb.ToBuffer()
	return buf.Flatten(), nil
}

func (t *virtualTun) Close() error {
	t.cancel()
	t.ep.Close()
	return nil
}
