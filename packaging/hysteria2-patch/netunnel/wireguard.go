package netunnel

import (
	"encoding/json"
	"fmt"
	"runtime"
	"sync"
	"sync/atomic"

	"github.com/amnezia-vpn/amneziawg-go/v3/conn"
	"github.com/amnezia-vpn/amneziawg-go/v3/device"
)

// WireGuardHandle — тоннель WireGuard / AmneziaWG (amneziawg-go). Без
// параметров маскировки в конфиге это обычный WireGuard (wg-easy), с ними
// — AmneziaWG. В отличие от hysteria2, здесь нет gVisor-стека и relay-
// соединений: WireGuard шифрует IP-пакеты целиком, поэтому памяти на
// iPhone нужно заметно меньше и relay-лимиты (handler.go) не участвуют.
type WireGuardHandle struct {
	device *device.Device
	tun    *wgTun

	txBytes uint64 // WritePacket: от ОС в тоннель — upload
	rxBytes uint64 // ReadPacket: из тоннеля в ОС — download

	history     *historyRecorder // nil, если HistoryPath пуст
	stopHistory chan struct{}
	stopOnce    sync.Once
}

// StartWireGuard поднимает WireGuard-устройство. Сокеты расширения идут
// мимо собственного тоннеля (так устроен NEPacketTunnelProvider), поэтому
// обычный UDP-bind до сервера работает без исключений маршрутов.
func StartWireGuard(configJSON string) (*WireGuardHandle, error) {
	cfg := WireGuardConfig{MemoryConstrained: true} // нет поля → безопасно для iPhone
	if err := json.Unmarshal([]byte(configJSON), &cfg); err != nil {
		return nil, fmt.Errorf("netunnel: bad wireguard config json: %w", err)
	}
	uapi, err := cfg.uapi()
	if err != nil {
		return nil, err
	}
	if cfg.MemoryConstrained {
		applyMemoryTuning()
	}
	mtu := cfg.MTU
	if mtu <= 0 {
		mtu = defaultWireGuardMTU
	}

	vtun := newWGTun(mtu)
	dev := device.NewDevice(vtun, conn.NewDefaultBind(), device.NewLogger(device.LogLevelError, "netunnel/wg: "))
	if err := dev.IpcSet(uapi); err != nil {
		dev.Close()
		return nil, fmt.Errorf("netunnel: wireguard config: %w", err)
	}
	if err := dev.Up(); err != nil {
		dev.Close()
		return nil, fmt.Errorf("netunnel: wireguard up: %w", err)
	}

	handle := &WireGuardHandle{device: dev, tun: vtun, stopHistory: make(chan struct{})}
	if cfg.HistoryPath != "" {
		handle.history = &historyRecorder{path: cfg.HistoryPath}
		go handle.history.recordPeriodically(&handle.txBytes, &handle.rxBytes, handle.stopHistory)
	}
	return handle, nil
}

func (h *WireGuardHandle) WritePacket(pkt []byte) error {
	atomic.AddUint64(&h.txBytes, uint64(len(pkt)))
	return h.tun.deliverInbound(pkt)
}

func (h *WireGuardHandle) ReadPacket() ([]byte, error) {
	pkt, err := h.tun.takeOutbound()
	if err == nil {
		atomic.AddUint64(&h.rxBytes, uint64(len(pkt)))
	}
	return pkt, err
}

// GetStats — тот же JSON, что TunnelHandle.GetStats; relay-счётчики у
// WireGuard нулевые (relay-соединений нет).
func (h *WireGuardHandle) GetStats() string {
	var m runtime.MemStats
	runtime.ReadMemStats(&m)
	return fmt.Sprintf(
		`{"txBytes":%d,"rxBytes":%d,"heapInUse":%d,"heapSys":%d,"goroutines":%d,"tcpRelays":0,"udpRelays":0,"registrySize":0,"availMem":%d}`,
		atomic.LoadUint64(&h.txBytes), atomic.LoadUint64(&h.rxBytes),
		m.HeapInuse, m.Sys, runtime.NumGoroutine(), availableMemoryBytes(),
	)
}

func (h *WireGuardHandle) FlushHistory() {
	h.history.flushCounters(&h.txBytes, &h.rxBytes)
}

// ForceReconnect — после сна устройства пересоздать UDP-сокеты: старые
// могли остаться привязанными к уже неактуальной сети (Wi-Fi ↔ сотовая).
// Рукопожатие WireGuard повторит сам при следующем пакете.
func (h *WireGuardHandle) ForceReconnect() {
	_ = h.device.BindUpdate()
}

func (h *WireGuardHandle) Stop() error {
	h.stopOnce.Do(func() {
		close(h.stopHistory)
		h.FlushHistory() // хвост сессии — до закрытия устройства
		h.device.Close() // закрывает и wgTun
	})
	return nil
}
