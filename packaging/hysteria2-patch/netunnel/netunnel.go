// Package netunnel — байт-слайс адаптация app/internal/tun для
// встраивания в среду без настоящего TUN-устройства: NEPacketTunnelProvider
// (macOS/iOS) отдаёт и принимает пакеты через NEPacketTunnelFlow.readPackets/
// writePackets, а не через файловый дескриптор. Собственный gVisor-стек
// (gvisor.dev/gvisor напрямую, см. virtual_tun.go/netunnel.go/handler.go) —
// НЕ через sing-tun: проверено вживую, `tun.NewSystem` форка
// apernet/sing-tun — это "System stack" (требует настоящего TUN-
// устройства с реально назначенным IP в ОС, у нас его нет), а
// `tun.NewGVisor` в этом форке вырезан целиком (заглушка с ошибкой).
//
// Методы TunnelHandle оперируют только []byte/строками/примитивами —
// gomobile bind не умеет маршалить произвольные Go-типы через границу
// с Swift (см. docs/ARCHITECTURE.md, раздел macOS/NetworkExtension).
//
// ⚠ НЕ ПРОВЕРЕНО НИ НА ОДНОЙ РЕАЛЬНОЙ ПЛАТФОРМЕ — только `go build`/
// `go vet` на Linux. Не проверено: gomobile-биндинг этого конкретного
// API (NSData*/NSError** на стороне Swift), реальный packet round-trip
// через NEPacketTunnelFlow, throughput/GC-нагрузка при реальной скорости
// пакетов.
//
// Паритет конфига с config_gen.rs (sidecar-путь): сделано — sni/insecure/
// pinSHA256, obfs (только salamander, gecko НЕ реализован — экспериментален
// и в самом upstream, см. bump.sh комментарий), bandwidth, congestion. НЕ
// сделано осознанно: quic-тюнинг (`Server.quic: HashMap<String,JsonValue>`
// в subscription.rs — произвольный passthrough с полями вида
// `initStreamReceiveWindow`/`maxIdleTimeout`, часть из них time.Duration,
// который `encoding/json` не парсит из строк "30s" так же, как mapstructure/
// viper в YAML-пути — риск тихо неправильно распарсить, не сделано вслепую)
// и transport.type=udphop (port-hopping — Config.Server резолвится только
// как обычный UDP-адрес через net.ResolveUDPAddr, не через udphop.ResolveUDPHopAddr).
package netunnel

import (
	"crypto/sha256"
	"crypto/x509"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"net"
	"net/netip"
	"strings"
	"sync"

	"gvisor.dev/gvisor/pkg/tcpip"
	"gvisor.dev/gvisor/pkg/tcpip/header"
	"gvisor.dev/gvisor/pkg/tcpip/network/ipv4"
	"gvisor.dev/gvisor/pkg/tcpip/network/ipv6"
	"gvisor.dev/gvisor/pkg/tcpip/stack"
	"gvisor.dev/gvisor/pkg/tcpip/transport/tcp"
	"gvisor.dev/gvisor/pkg/tcpip/transport/udp"

	"github.com/apernet/hysteria/app/v2/internal/utils"
	"github.com/apernet/hysteria/core/v2/client"
	"github.com/apernet/hysteria/extras/v2/obfs"
)

// nicID — единственный NIC в нашем gVisor-стеке (точка-точка, один
// virtualTun на одно соединение).
const nicID tcpip.NICID = 1

// Config — JSON-конфиг для StartTunnel, по полям зеркалит то, что
// config_gen.rs строит для sidecar-пути (см. doc-комментарий пакета про
// то, что осознанно не перенесено). inet4Addr/inet6Addr — CIDR, из
// которого берётся только сам адрес (Addr()) — собственному gVisor-NIC
// нужен ровно один адрес, второй "свободный" (как требовал sing-tun's
// System stack) здесь не нужен — нет настоящего TUN-устройства и
// настоящего соседа по подсети, NIC сам является единственной точкой
// входа для всего трафика (promiscuous + spoofing, см. StartTunnel).
type Config struct {
	Server     string           `json:"server"`
	Auth       string           `json:"auth"`
	SNI        string           `json:"sni"`
	Insecure   bool             `json:"insecure"`
	PinSHA256  string           `json:"pinSHA256,omitempty"`
	Obfs       ObfsConfig       `json:"obfs"`
	Bandwidth  BandwidthConfig  `json:"bandwidth"`
	Congestion CongestionConfig `json:"congestion"`
	Inet4Addr  string           `json:"inet4Addr"`
	Inet6Addr  string           `json:"inet6Addr,omitempty"`
	MTU        uint32           `json:"mtu"`
}

type ObfsConfig struct {
	Type       string `json:"type"`
	Salamander struct {
		Password string `json:"password"`
	} `json:"salamander"`
}

type BandwidthConfig struct {
	Up   string `json:"up"`
	Down string `json:"down"`
}

type CongestionConfig struct {
	Type       string `json:"type"`
	BBRProfile string `json:"bbrProfile"`
}

// TunnelHandle — gomobile-совместимый хендл одного активного соединения.
type TunnelHandle struct {
	vtun   *virtualTun
	stack  *stack.Stack
	client client.Client
}

// normalizeCertHash — копия app/cmd/client.go::normalizeCertHash (не
// импортирована: функция unexported в package main).
func normalizeCertHash(hash string) string {
	r := strings.ToLower(hash)
	r = strings.ReplaceAll(r, ":", "")
	r = strings.ReplaceAll(r, "-", "")
	return r
}

// singleUseConnFactory — упрощённая копия app/cmd/client.go::
// singleUseConnFactory (тоже unexported в package main): открывает один
// UDP-сокет и оборачивает его в obfs, если задан. Без port-hopping и
// quic.sockopts (bindInterface/fwmark) — для встроенного в NE-расширение
// клиента они не имеют смысла (нет привилегированного доступа к сетевым
// интерфейсам так, как на Linux/в sidecar-модели).
type singleUseConnFactory struct {
	obfsType     string
	obfsPassword string

	mu   sync.Mutex
	used bool
}

func (f *singleUseConnFactory) New(net.Addr) (net.PacketConn, error) {
	f.mu.Lock()
	defer f.mu.Unlock()
	if f.used {
		return nil, errors.New("netunnel: connection factory already used")
	}
	f.used = true

	conn, err := net.ListenUDP("udp", nil)
	if err != nil {
		return nil, err
	}
	switch strings.ToLower(f.obfsType) {
	case "", "plain":
		return conn, nil
	case "salamander":
		wrapped, wrapErr := obfs.WrapPacketConnSalamander(conn, []byte(f.obfsPassword))
		if wrapErr != nil {
			_ = conn.Close()
			return nil, wrapErr
		}
		return wrapped, nil
	default:
		_ = conn.Close()
		return nil, fmt.Errorf("netunnel: obfs type %q не реализован (только salamander/plain)", f.obfsType)
	}
}

func buildClientConfig(cfg *Config) (*client.Config, error) {
	if cfg.Server == "" {
		return nil, fmt.Errorf("netunnel: server is required")
	}
	serverAddr, err := net.ResolveUDPAddr("udp", cfg.Server)
	if err != nil {
		return nil, fmt.Errorf("netunnel: resolve server addr: %w", err)
	}
	sni := cfg.SNI
	if sni == "" {
		host, _, splitErr := net.SplitHostPort(cfg.Server)
		if splitErr == nil {
			sni = host
		}
	}

	hyConfig := &client.Config{
		ServerAddr: serverAddr,
		Auth:       cfg.Auth,
		TLSConfig: client.TLSConfig{
			ServerName:         sni,
			InsecureSkipVerify: cfg.Insecure,
		},
		CongestionConfig: client.CongestionConfig{
			Type:       cfg.Congestion.Type,
			BBRProfile: cfg.Congestion.BBRProfile,
		},
		ConnFactory: &singleUseConnFactory{
			obfsType:     cfg.Obfs.Type,
			obfsPassword: cfg.Obfs.Salamander.Password,
		},
	}

	if cfg.PinSHA256 != "" {
		nHash := normalizeCertHash(cfg.PinSHA256)
		hyConfig.TLSConfig.VerifyPeerCertificate = func(rawCerts [][]byte, _ [][]*x509.Certificate) error {
			cert := rawCerts[0]
			hash := sha256.Sum256(cert)
			if hex.EncodeToString(hash[:]) == nHash {
				return nil
			}
			return errors.New("netunnel: no certificate matches the pinned hash")
		}
	}

	if cfg.Bandwidth.Up != "" {
		maxTx, convErr := utils.ConvBandwidth(cfg.Bandwidth.Up)
		if convErr != nil {
			return nil, fmt.Errorf("netunnel: bandwidth.up: %w", convErr)
		}
		hyConfig.BandwidthConfig.MaxTx = maxTx
	}
	if cfg.Bandwidth.Down != "" {
		maxRx, convErr := utils.ConvBandwidth(cfg.Bandwidth.Down)
		if convErr != nil {
			return nil, fmt.Errorf("netunnel: bandwidth.down: %w", convErr)
		}
		hyConfig.BandwidthConfig.MaxRx = maxRx
	}

	return hyConfig, nil
}

// StartTunnel парсит configJSON, поднимает hysteria2-клиент и gVisor-стек
// поверх virtualTun (без настоящего TUN-устройства). Возвращённый хендл
// готов сразу принимать WritePacket/отдавать ReadPacket — Start() стека
// не блокирует (запускает свой цикл в фоне), в отличие от Run(), который
// использует app/internal/tun/server.go для sidecar-пути.
func StartTunnel(configJSON string) (*TunnelHandle, error) {
	var cfg Config
	if err := json.Unmarshal([]byte(configJSON), &cfg); err != nil {
		return nil, fmt.Errorf("netunnel: bad config json: %w", err)
	}

	hyConfig, err := buildClientConfig(&cfg)
	if err != nil {
		return nil, err
	}
	hyClient, _, err := client.NewClient(hyConfig)
	if err != nil {
		return nil, fmt.Errorf("netunnel: hysteria client: %w", err)
	}

	inet4, err := netip.ParsePrefix(cfg.Inet4Addr)
	if err != nil {
		_ = hyClient.Close()
		return nil, fmt.Errorf("netunnel: bad inet4Addr: %w", err)
	}
	var inet6 netip.Prefix
	hasInet6 := false
	if cfg.Inet6Addr != "" {
		inet6, err = netip.ParsePrefix(cfg.Inet6Addr)
		if err != nil {
			_ = hyClient.Close()
			return nil, fmt.Errorf("netunnel: bad inet6Addr: %w", err)
		}
		hasInet6 = true
	}

	mtu := cfg.MTU
	if mtu == 0 {
		mtu = 1500
	}
	vtun := newVirtualTun(mtu)

	netStack := stack.New(stack.Options{
		NetworkProtocols:   []stack.NetworkProtocolFactory{ipv4.NewProtocol, ipv6.NewProtocol},
		TransportProtocols: []stack.TransportProtocolFactory{tcp.NewProtocol, udp.NewProtocol},
	})
	if err := netStack.CreateNIC(nicID, vtun.ep); err != nil {
		_ = hyClient.Close()
		return nil, fmt.Errorf("netunnel: create NIC: %s", err)
	}
	// promiscuous + spoofing: NIC должен принимать и отправлять пакеты с
	// адресами, которые не совпадают с его собственным — у нас точка-
	// точка "тоннель в одно лицо", через этот единственный NIC идёт
	// трафик к ЛЮБЫМ адресам назначения в интернете, не только к
	// собственному IP интерфейса (как было бы у обычной NIC с реальным
	// соседом по L2).
	if err := netStack.SetPromiscuousMode(nicID, true); err != nil {
		_ = hyClient.Close()
		return nil, fmt.Errorf("netunnel: set promiscuous mode: %s", err)
	}
	if err := netStack.SetSpoofing(nicID, true); err != nil {
		_ = hyClient.Close()
		return nil, fmt.Errorf("netunnel: set spoofing: %s", err)
	}

	if err := netStack.AddProtocolAddress(nicID, tcpip.ProtocolAddress{
		Protocol:          ipv4.ProtocolNumber,
		AddressWithPrefix: tcpip.AddrFromSlice(inet4.Addr().AsSlice()).WithPrefix(),
	}, stack.AddressProperties{}); err != nil {
		_ = hyClient.Close()
		return nil, fmt.Errorf("netunnel: add IPv4 address: %s", err)
	}
	routes := []tcpip.Route{{Destination: header.IPv4EmptySubnet, NIC: nicID}}
	if hasInet6 {
		if err := netStack.AddProtocolAddress(nicID, tcpip.ProtocolAddress{
			Protocol:          ipv6.ProtocolNumber,
			AddressWithPrefix: tcpip.AddrFromSlice(inet6.Addr().AsSlice()).WithPrefix(),
		}, stack.AddressProperties{}); err != nil {
			_ = hyClient.Close()
			return nil, fmt.Errorf("netunnel: add IPv6 address: %s", err)
		}
		routes = append(routes, tcpip.Route{Destination: header.IPv6EmptySubnet, NIC: nicID})
	}
	netStack.SetRouteTable(routes)

	tcpForwarder := tcp.NewForwarder(netStack, 0, 1024, tcpForwarderHandler(hyClient))
	netStack.SetTransportProtocolHandler(tcp.ProtocolNumber, tcpForwarder.HandlePacket)
	udpForwarder := udp.NewForwarder(netStack, udpForwarderHandler(hyClient))
	netStack.SetTransportProtocolHandler(udp.ProtocolNumber, udpForwarder.HandlePacket)

	return &TunnelHandle{vtun: vtun, stack: netStack, client: hyClient}, nil
}

// WritePacket — пакет ОТ Swift (NEPacketTunnelFlow.readPackets), отдаём
// в gVisor stack как будто он пришёл из настоящего TUN.
func (h *TunnelHandle) WritePacket(pkt []byte) error {
	return h.vtun.deliverInbound(pkt)
}

// ReadPacket — блокируется до следующего пакета, который gVisor stack
// хочет отправить К Swift (NEPacketTunnelFlow.writePackets), либо до Stop().
func (h *TunnelHandle) ReadPacket() ([]byte, error) {
	return h.vtun.takeOutbound()
}

func (h *TunnelHandle) Stop() error {
	h.stack.Close()
	_ = h.vtun.Close()
	return h.client.Close()
}
