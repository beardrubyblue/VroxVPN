package netunnel

import (
	"context"
	"io"
	"net"
	"net/netip"

	"github.com/sagernet/sing/common/buf"
	"github.com/sagernet/sing/common/metadata"
	"github.com/sagernet/sing/common/network"

	"github.com/apernet/hysteria/core/v2/client"
)

// relayHandler — реализация tun.Handler. Логика relay (дозвон через
// HyClient.TCP/UDP + io.Copy) скопирована из app/internal/tun/server.go
// (tunHandler.NewConnection/NewPacketConnection), а не импортирована
// оттуда: tunHandler там unexported (package tun), тащить отдельный
// экспортированный API в патч ради этого не стали — решили не трогать
// сильнее, чем нужно, уже патченый upstream-файл.
//
// directDomains (обход VPN по доменам через DNS-сниффинг) здесь
// СОЗНАТЕЛЬНО не перенесён — по плану (docs/ARCHITECTURE.md, раздел
// macOS/NetworkExtension, Фаза 3) на NE-пути это становится статическим
// excludedRoutes на стороне Rust (config_gen.rs), а не runtime-сниффингом:
// NEPacketTunnelProvider не даёt доступа к реальному физическому
// интерфейсу так, как это нужно текущему dialDirect/startDNSSniffer.
type relayHandler struct {
	hyClient client.Client
}

func (h *relayHandler) NewConnection(ctx context.Context, conn net.Conn, m metadata.Metadata) error {
	reqAddr := m.Destination.String()
	hyConn, err := h.hyClient.TCP(reqAddr)
	if err != nil {
		return nil //nolint:nilerr // как и в server.go — ошибка не всплывает наверх по контракту tun.Handler
	}
	defer hyConn.Close()

	copyErrChan := make(chan error, 2)
	go func() {
		_, copyErr := io.Copy(hyConn, conn)
		copyErrChan <- copyErr
	}()
	go func() {
		_, copyErr := io.Copy(conn, hyConn)
		copyErrChan <- copyErr
	}()
	select {
	case <-ctx.Done():
	case <-copyErrChan:
	}
	return nil
}

func (h *relayHandler) NewPacketConnection(ctx context.Context, conn network.PacketConn, m metadata.Metadata) error {
	rc, err := h.hyClient.UDP()
	if err != nil {
		return nil //nolint:nilerr
	}
	defer rc.Close()

	copyErrChan := make(chan error, 2)
	// local <- remote
	go func() {
		for {
			bs, from, err := rc.Receive()
			if err != nil {
				copyErrChan <- err
				return
			}
			var fromAddr metadata.Socksaddr
			if ap, perr := netip.ParseAddrPort(from); perr == nil {
				fromAddr = metadata.SocksaddrFromNetIP(ap)
			} else {
				fromAddr.Fqdn = from
			}
			if err := conn.WritePacket(buf.As(bs), fromAddr); err != nil {
				copyErrChan <- err
				return
			}
		}
	}()
	// local -> remote
	go func() {
		buffer := buf.NewPacket()
		defer buffer.Release()
		for {
			buffer.Reset()
			addr, err := conn.ReadPacket(buffer)
			if err != nil {
				copyErrChan <- err
				return
			}
			if err := rc.Send(buffer.Bytes(), addr.String()); err != nil {
				copyErrChan <- err
				return
			}
		}
	}()
	select {
	case <-ctx.Done():
	case <-copyErrChan:
	}
	return nil
}

func (h *relayHandler) NewError(ctx context.Context, err error) {
	// как и в server.go — намеренно не используется
}
