// Package netunnel — байт-слайс адаптация app/internal/tun для
// встраивания в среду без настоящего TUN-устройства: NEPacketTunnelProvider
// (macOS/iOS) отдаёт и принимает пакеты через NEPacketTunnelFlow.readPackets/
// writePackets, а не через файловый дескриптор. virtualTun (virtual_tun.go)
// подсовывает gVisor-стеку (tun.NewSystem) канал вместо fd — сам стек и
// relay-логика (handler.go) от этого не меняются.
//
// Методы TunnelHandle оперируют только []byte/строками/примитивами —
// gomobile bind не умеет маршалить произвольные Go-типы через границу
// с Swift (см. docs/ARCHITECTURE.md, раздел macOS/NetworkExtension).
//
// ⚠ НЕ ПРОВЕРЕНО НИ НА ОДНОЙ РЕАЛЬНОЙ ПЛАТФОРМЕ — только `go build`/
// `go vet` на Linux. Не проверено: gomobile-биндинг этого конкретного
// API (NSData*/NSError** на стороне Swift), реальный packet round-trip
// через NEPacketTunnelFlow, throughput/GC-нагрузка при реальной скорости
// пакетов. Конфиг (Config ниже) — минимальный: server/auth/sni/insecure,
// БЕЗ obfs/QUIC-тюнинга, который есть в config_gen.rs для sidecar-пути —
// полный паритет полей это отдельный шаг (Фаза 5 плана), не сделан
// сейчас, потому что сначала важно было проверить саму архитектуру
// (virtualTun + gVisor stack без реального fd), а не покрыть все поля.
package netunnel

import (
	"context"
	"encoding/json"
	"fmt"
	"net"
	"net/netip"

	tun "github.com/apernet/sing-tun"
	"github.com/sagernet/sing/common/logger"

	"github.com/apernet/hysteria/core/v2/client"
)

// Config — минимальный JSON-конфиг для StartTunnel. inet4Addr/inet6Addr —
// тот же формат, что уже строит config_gen.rs (CIDR с серверным адресом
// первым, см. tun.address в YAML для sidecar-пути) — gVisor-стеку нужен
// один свободный адрес после сетевого для самого клиента (см. проверку
// "need one more IPv4 address" в sing-tun/stack_system.go).
type Config struct {
	Server    string `json:"server"`
	Auth      string `json:"auth"`
	SNI       string `json:"sni"`
	Insecure  bool   `json:"insecure"`
	Inet4Addr string `json:"inet4Addr"`
	Inet6Addr string `json:"inet6Addr,omitempty"`
	MTU       uint32 `json:"mtu"`
}

// TunnelHandle — gomobile-совместимый хендл одного активного соединения.
type TunnelHandle struct {
	vtun   *virtualTun
	stack  tun.Stack
	client client.Client
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
	return &client.Config{
		ServerAddr: serverAddr,
		Auth:       cfg.Auth,
		TLSConfig: client.TLSConfig{
			ServerName:         sni,
			InsecureSkipVerify: cfg.Insecure,
		},
	}, nil
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
	var inet6Prefixes []netip.Prefix
	if cfg.Inet6Addr != "" {
		inet6, parseErr := netip.ParsePrefix(cfg.Inet6Addr)
		if parseErr != nil {
			_ = hyClient.Close()
			return nil, fmt.Errorf("netunnel: bad inet6Addr: %w", parseErr)
		}
		inet6Prefixes = []netip.Prefix{inet6}
	}

	vtun := newVirtualTun()
	stack, err := tun.NewSystem(tun.StackOptions{
		Context: context.Background(),
		Tun:     vtun,
		TunOptions: tun.Options{
			Inet4Address: []netip.Prefix{inet4},
			Inet6Address: inet6Prefixes,
			MTU:          cfg.MTU,
		},
		Handler: &relayHandler{hyClient: hyClient},
		Logger:  logger.NOP(),
	})
	if err != nil {
		_ = hyClient.Close()
		return nil, fmt.Errorf("netunnel: stack: %w", err)
	}
	if err := stack.Start(); err != nil {
		_ = hyClient.Close()
		return nil, fmt.Errorf("netunnel: stack start: %w", err)
	}

	return &TunnelHandle{vtun: vtun, stack: stack, client: hyClient}, nil
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
	_ = h.stack.Close()
	_ = h.vtun.Close()
	return h.client.Close()
}
