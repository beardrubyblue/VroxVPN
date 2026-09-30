package netunnel

import (
	"encoding/json"
	"fmt"
	"runtime"
	"strconv"
	"strings"
	"sync"
	"sync/atomic"
	"time"

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

	// последняя ошибка WireGuard (логгер устройства) — для экрана Stats:
	// логи расширения на iPhone больше никуда не попадают
	errorMu   sync.Mutex
	lastError string
}

func (h *WireGuardHandle) recordError(format string, args ...any) {
	h.errorMu.Lock()
	h.lastError = fmt.Sprintf(format, args...)
	h.errorMu.Unlock()
}

// handshakeAgeSec — сколько секунд назад было последнее рукопожатие с
// сервером; -1 — ни одного (сервер не отвечает, сеть режет WireGuard).
func (h *WireGuardHandle) handshakeAgeSec() int64 {
	state, err := h.device.IpcGet()
	if err != nil {
		return -1
	}
	for _, line := range strings.Split(state, "\n") {
		if value, ok := strings.CutPrefix(line, "last_handshake_time_sec="); ok {
			if sec, err := strconv.ParseInt(value, 10, 64); err == nil && sec > 0 {
				return time.Now().Unix() - sec
			}
		}
	}
	return -1
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
	handle := &WireGuardHandle{tun: vtun, stopHistory: make(chan struct{})}
	logger := &device.Logger{Verbosef: device.DiscardLogf, Errorf: handle.recordError}
	dev := device.NewDevice(vtun, conn.NewDefaultBind(), logger)
	handle.device = dev
	if err := dev.IpcSet(uapi); err != nil {
		dev.Close()
		return nil, fmt.Errorf("netunnel: wireguard config: %w", err)
	}
	if err := dev.Up(); err != nil {
		dev.Close()
		return nil, fmt.Errorf("netunnel: wireguard up: %w", err)
	}

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

// GetStats — те же поля, что TunnelHandle.GetStats (relay-счётчики у
// WireGuard нулевые), плюс диагностика WireGuard: возраст рукопожатия и
// последняя ошибка (engine/macos/stats.rs → экран Stats).
func (h *WireGuardHandle) GetStats() string {
	var m runtime.MemStats
	runtime.ReadMemStats(&m)
	h.errorMu.Lock()
	lastError := h.lastError
	h.errorMu.Unlock()
	stats, _ := json.Marshal(map[string]any{
		"txBytes": atomic.LoadUint64(&h.txBytes), "rxBytes": atomic.LoadUint64(&h.rxBytes),
		"heapInUse": m.HeapInuse, "heapSys": m.Sys, "goroutines": runtime.NumGoroutine(),
		"tcpRelays": 0, "udpRelays": 0, "registrySize": 0, "availMem": availableMemoryBytes(),
		"handshakeAgeSec": h.handshakeAgeSec(), "wgError": lastError,
	})
	return string(stats)
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
