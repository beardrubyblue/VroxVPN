package netunnel

import (
	"encoding/json"
	"fmt"
)

// protocolWireGuard — значение поля "protocol" конфига для WireGuard /
// AmneziaWG; пусто или что угодно другое — hysteria2 (исторически
// единственный протокол, поля "protocol" в его конфиге нет).
const protocolWireGuard = "wireguard"

// Tunnel — то, что Swift (PacketTunnelProvider) делает с тоннелем любого
// протокола. gomobile экспортирует интерфейс как Obj-C протокол, так что
// Swift не знает, hysteria2 внутри или WireGuard.
type Tunnel interface {
	// WritePacket — пакет ОТ ОС (NEPacketTunnelFlow.readPackets) в тоннель.
	WritePacket(pkt []byte) error
	// ReadPacket — блокируется до пакета ИЗ тоннеля к ОС либо до Stop().
	ReadPacket() ([]byte, error)
	// GetStats — JSON со счётчиками трафика и памяти (engine/macos/stats.rs).
	GetStats() string
	// FlushHistory — дописать накопленный трафик в историю (history.go).
	FlushHistory()
	// ForceReconnect — устройство проснулось, соединение, скорее всего, мертво.
	ForceReconnect()
	Stop() error
}

// Start — единая точка входа для Swift: протокол по полю "protocol".
func Start(configJSON string) (Tunnel, error) {
	var head struct {
		Protocol string `json:"protocol"`
	}
	if err := json.Unmarshal([]byte(configJSON), &head); err != nil {
		return nil, fmt.Errorf("netunnel: bad config json: %w", err)
	}
	// Явные nil-проверки: typed-nil указатель, завёрнутый в интерфейс, не
	// равен nil — Swift получил бы «успех» с мёртвым хэндлом.
	if head.Protocol == protocolWireGuard {
		handle, err := StartWireGuard(configJSON)
		if err != nil {
			return nil, err
		}
		return handle, nil
	}
	handle, err := StartTunnel(configJSON)
	if err != nil {
		return nil, err
	}
	return handle, nil
}
