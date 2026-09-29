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
	"runtime"
	"runtime/debug"
	"strings"
	"sync"
	"sync/atomic"
	"time"

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

// iOS убивает NEPacketTunnelProvider при превышении ~50МБ RSS (жёсткий
// OS-лимит, не настраиваемый нами) — пойман вживую: реальный спидтест
// разгонял память до ~59МБ, расширение падало посреди соединения.
// Go по умолчанию не торопится отдавать память ОС обратно после всплеска
// (GC планирует следующий цикл по эвристике 2× от живых данных, а не по
// абсолютному потолку) — debug.SetMemoryLimit задаёт мягкий потолок
// кучи, после которого GC запускается агрессивнее, не дожидаясь
// штатного триггера. 30 МиБ — с запасом до 50, остальное (gVisor-стек,
// QUIC-буфера, сам Go-рантайм) пока не профилировано отдельно, есть куда
// тюнить дальше, если этого не хватит. GOGC=20 — то же самое, но по
// относительному приросту, а не абсолютному потолку, дополняет, не
// дублирует SetMemoryLimit (GC сработает от того лимита, который
// сработает раньше).
//
// Применяется только при Config.MemoryConstrained (см. там) — поэтому
// это не init(), а вызов из StartTunnel.
func applyMemoryTuning() {
	// Каждый Go-поток (M в терминологии рантайма) — это отдельный OS-
	// тред со своим стеком плюс per-P аллокаторские кеши (mcache).
	// Расширению не нужен параллелизм между ядрами — вся работа уже
	// размазана по горутинам с своим планировщиком внутри одного потока;
	// GOMAXPROCS=1 убирает лишние OS-потоки и их кеши, не трогая логику.
	runtime.GOMAXPROCS(1)
	debug.SetMemoryLimit(30 << 20) // 30 МиБ
	debug.SetGCPercent(20)
}

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
	// MemoryConstrained — работаем ли под iOS jetsam (~50МБ). Выставляет
	// Swift-расширение по окружению (PacketTunnelProvider.swift::
	// isMemoryConstrained), не Rust: iOS-сборка из TestFlight ставится и
	// на Apple Silicon Mac (isiOSAppOnMac) — компилируется как iOS, но
	// jetsam-потолка там нет. Раньше это решалось build-тегом, и на Mac
	// работали iOS-механизмы: reconnectPeriodically каждые 3 мин эвиктил
	// ВСЕ relay-соединения (приложение Claude бесконечно
	// переподключалось), лимиты 30с/64/32 давали RST новым соединениям
	// (ERR_CONNECTION_CLOSED в браузере). false → GOMAXPROCS/лимит кучи
	// не трогаем, фоновые механизмы не запускаем, relay-лимиты
	// десктопные (applyRelayLimits). Нет поля в JSON → true (StartTunnel):
	// безопаснее перестраховаться памятью, чем получить jetsam на iPhone.
	MemoryConstrained bool `json:"memoryConstrained"`
	// HistoryPath — файл истории трафика по дням в App Group (history.go);
	// выставляет Swift. Пусто — история не пишется.
	HistoryPath string `json:"historyPath,omitempty"`
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
//
// txBytes/rxBytes — счётчики трафика для UI (см. docs/ARCHITECTURE.md,
// раздел про traffic stats). Считаются на границе WritePacket/ReadPacket
// (Swift↔Go), а не внутри relayTCP/relayUDP (handler.go) — это
// единственная точка, через которую проходит вообще весь трафик тоннеля
// в обе стороны, независимо от протокола, ровно как раньше на Linux
// считались rx/tx на самом tun-интерфейсе (core/stats.py::
// _read_interface_bytes), а не на отдельных соединениях.
type TunnelHandle struct {
	vtun     *virtualTun
	stack    *stack.Stack
	hyConfig *client.Config // для периодического реконнекта

	obfsType     string // для пересоздания ConnFactory при реконнекте
	obfsPassword string

	clientMu sync.RWMutex
	client   client.Client

	reconnectMu  sync.Mutex  // сериализует вызовы reconnectClient
	reconnecting atomic.Bool // дедупликация триггеров maybeReconnect (не плодить горутины)

	// dialFailStreak — сколько подряд hyClient.TCP/UDP() вернули ошибку.
	// Главный детектор "тоннель умер, пока телефон спал": после
	// разблокировки приложения шлют пакеты в мёртвый QUIC, дозвон
	// фейлит пачкой — при превышении порога форсим реконнект НЕ дожидаясь
	// периодического таймера (тот к тому же не идёт, пока процесс
	// заморожен iOS). Сбрасывается в 0 на первый же успешный дозвон.
	dialFailStreak atomic.Int32

	txBytes uint64 // WritePacket: пакеты ОТ ОС, "наружу" через тоннель — upload
	rxBytes uint64 // ReadPacket: пакеты К ОС, "из" тоннеля — download

	stopMemoryReclaim chan struct{}

	history     *historyRecorder // nil, если Config.HistoryPath пуст
	stopHistory chan struct{}
}

// getClient — потокобезопасный доступ к текущему hysteria-клиенту.
// Forwarder-хэндлеры (handler.go) вызывают его на каждое новое
// соединение, а не захватывают client в замыкание при создании
// forwarder'а — это позволяет reconnectClient() подменить клиент
// на лету, без пересоздания gVisor-стека.
func (h *TunnelHandle) getClient() client.Client {
	h.clientMu.RLock()
	c := h.client
	h.clientMu.RUnlock()
	return c
}

// reconnectClient — пересоздаёт QUIC-соединение к серверу, сбрасывая
// ВСЮ накопленную память quic-go (stream tracking, flow control windows,
// internal buffers). Главный рычаг против "память растёт пока не
// сдохнет": единственное QUIC-соединение тоннеля копит состояние
// пропорционально числу streams, которые когда-либо через него прошли
// — ни GC, ни FreeOSMemory это не освободят, потому что ссылки живые.
// Пересоздание занимает ~1 RTT (30–100мс), во время которого новые
// relay получат ошибку и пересоздадутся — кратковременный stutter.
func (h *TunnelHandle) reconnectClient() error {
	h.reconnectMu.Lock()
	defer h.reconnectMu.Unlock()

	type result struct {
		c   client.Client
		err error
	}
	ch := make(chan result, 1)
	go func() {
		h.hyConfig.ConnFactory = &singleUseConnFactory{
			obfsType:     h.obfsType,
			obfsPassword: h.obfsPassword,
		}
		newClient, _, err := client.NewClient(h.hyConfig)
		ch <- result{newClient, err}
	}()

	var res result
	select {
	case res = <-ch:
	case <-time.After(10 * time.Second):
		return errors.New("reconnect: timeout")
	case <-h.stopMemoryReclaim:
		return errors.New("reconnect: stopped")
	}
	if res.err != nil {
		return fmt.Errorf("reconnect: %w", res.err)
	}

	// Подменяем клиент атомарно — новые relay сразу пойдут через
	// новое QUIC-соединение
	h.clientMu.Lock()
	oldClient := h.client
	h.client = res.c
	h.clientMu.Unlock()

	// Эвиктим ВСЕ старые соединения (они держат ссылки на streams
	// старого клиента)
	for evictOldestConn() {
	}

	_ = oldClient.Close()
	h.dialFailStreak.Store(0)

	runtime.GC()
	debug.FreeOSMemory()
	return nil
}

// maybeReconnect запускает реконнект в фоне, но не больше одного за раз
// (reconnecting-флаг) — много forwarder-горутин могут разом обнаружить
// мёртвый тоннель и все позвать сюда; без дедупликации это сотни горутин
// на reconnectMu. Неблокирующий: вызывается из горячих путей (forwarder,
// тикеры, wake).
func (h *TunnelHandle) maybeReconnect() {
	if h.reconnecting.CompareAndSwap(false, true) {
		go func() {
			defer h.reconnecting.Store(false)
			_ = h.reconnectClient()
		}()
	}
}

// ForceReconnect — точка входа для Swift (PacketTunnelProvider.wake()):
// iOS будит расширение после сна устройства, и тоннель почти наверняка
// мёртв (NAT-маппинг UDP истёк за время заморозки процесса) — форсим
// пересоздание QUIC немедленно, не дожидаясь, пока приложения
// натолкнутся на мёртвый дозвон. gomobile экспортирует как
// forceReconnect().
func (h *TunnelHandle) ForceReconnect() {
	h.maybeReconnect()
}

// reconnectFailThreshold — сколько подряд неудачных дозвонов считаем
// признаком мёртвого тоннеля. 3 — достаточно мало для быстрого
// восстановления после сна, но не реагирует на одиночный отказ сервера
// по конкретному соединению (нормальный сетевой шум).
const reconnectFailThreshold = 3

// noteDialResult вызывается forwarder'ами (handler.go) после каждой
// попытки hyClient.TCP/UDP(): успех сбрасывает streak, ошибка копит его
// и при достижении порога форсит реконнект.
func (h *TunnelHandle) noteDialResult(ok bool) {
	if ok {
		h.dialFailStreak.Store(0)
		return
	}
	if h.dialFailStreak.Add(1) >= reconnectFailThreshold {
		h.maybeReconnect()
	}
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
		// НЕ настроено по умолчанию в core/client/config.go —
		// MaxStreamReceiveWindow дефолтится в 8МБ НА КАЖДЫЙ stream (а
		// stream = одно relayTCP-соединение, реальный сайт открывает
		// их десятками), MaxConnectionReceiveWindow — в 20МБ суммарно
		// на всё QUIC-соединение. Это и оказалось основным потребителем
		// памяти под нагрузкой, не gVisor TCP-буфера (см. соседний
		// SetTransportProtocolOption в StartTunnel — тот правит только
		// локальный virtual-TCP, не сам тоннель к серверу). Снижено с
		// большим запасом до бюджета iOS NE (~50МБ, см. init() ниже).
		// Второй проход на снижение: QUIC flow-control окна растут под
		// наблюдаемый throughput, но НЕ сжимаются обратно в рамках
		// жизни соединения (стандартное поведение QUIC/HTTP-стилей
		// flow control) — а у нас ОДНО QUIC-соединение на весь тоннель,
		// все relay-потоки мультиплексируются через него. Поймано
		// вживую: после разгона видео (Instagram Reels) пик 48МБ
		// держался даже спустя >60с простоя — не падал, пока не
		// переподключишь тоннель целиком. 4МБ/1МБ (первый проход) явно
		// недостаточно тесно для видео-throughput. Третий проход: пик
		// 26.5МБ (видео) всё ещё не падал без полного переподключения
		// тоннеля — ожидаемо, окно живёт, пока жив сам QUIC-коннект к
		// серверу (один на весь тоннель). Дальнейшее ужимание снижает
		// потолок ценой риска подвисаний при быстрой прокрутке видео.
		// Четвёртый проход — пользователь подтвердил вживую, что 256КБ/
		// 1МБ не давали подвисаний на видео, попросил ужать ещё. Пятый
		// проход: скорость загрузки почти не отличалась от четвёртого —
		// запас всё ещё есть, ужимаем до практического пола. 16384 —
		// минимум, который принимает hysteria2 для Initial*-полей (см.
		// core/client/config.go::verifyAndFill, "must be at least
		// 16384") — ниже этого библиотека просто вернёт ошибку конфига.
		// Проблема "накапливается и не падает при долгой прокрутке" —
		// НЕ про потолок окна (он теперь маленький), а про то, что
		// единственное QUIC-соединение тоннеля само НЕ сжимает окно
		// обратно в рамках своей жизни — это снижает максимум, но не
		// лечит сам факт накопления при достаточно долгой нагрузке
		// (нужен будет либо периодический реконнект, либо патч
		// AllowConnectionWindowIncrease в форке — отложено, см. историю
		// обсуждения).
		QUICConfig: client.QUICConfig{
			InitialStreamReceiveWindow:     64 << 10,  // 64 КиБ
			MaxStreamReceiveWindow:         512 << 10, // 512 КиБ
			InitialConnectionReceiveWindow: 256 << 10, // 256 КиБ
			MaxConnectionReceiveWindow:     2 << 20,   // 2 МиБ — ~500 Мбит/с при RTT 30мс
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
	cfg := Config{MemoryConstrained: true}
	if err := json.Unmarshal([]byte(configJSON), &cfg); err != nil {
		return nil, fmt.Errorf("netunnel: bad config json: %w", err)
	}
	if cfg.MemoryConstrained {
		applyMemoryTuning()
	}
	applyRelayLimits(&cfg)

	hyConfig, err := buildClientConfig(&cfg)
	if err != nil {
		return nil, err
	}
	// client.NewClient не принимает context — это синхронный вызов
	// (внутри сразу делает реальный QUIC-дозвон, не лениво), без
	// внешнего таймаута своего собственного. Живым тестом подтверждено,
	// что сам коннект через netunnel реально работает (curl по IP через
	// тоннель — успех), но иногда хендшейк зависал намного дольше, чем
	// MaxIdleTimeout=30с по умолчанию (core/client/config.go::
	// verifyAndFill) — внешний таймаут здесь просто страховка на этот
	// случай, чтобы UI получал внятную ошибку, а не тишину навсегда.
	type clientResult struct {
		client client.Client
		err    error
	}
	resultCh := make(chan clientResult, 1)
	go func() {
		hyClient, _, clientErr := client.NewClient(hyConfig)
		resultCh <- clientResult{client: hyClient, err: clientErr}
	}()

	var hyClient client.Client
	select {
	case res := <-resultCh:
		if res.err != nil {
			return nil, fmt.Errorf("netunnel: hysteria client: %w", res.err)
		}
		hyClient = res.client
	case <-time.After(20 * time.Second):
		// горутина выше продолжит висеть в фоне (NewClient не отменяем,
		// у него нет context) — известная утечка на этот случай, не
		// страшно: TunnelHandle всё равно не создаётся, повторный
		// StartTunnel запустит новую попытку независимо от этой.
		return nil, errors.New("netunnel: hysteria client: таймаут 20с — зависание ДО QUIC-хендшейка (ConnFactory/net.ListenUDP?), не сетевая проблема на уровне QUIC")
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

	// gVisor по умолчанию авто-тюнит TCP-буфера ВВЕРХ под наблюдаемый
	// throughput/RTT — у настоящего сайта десятки параллельных TCP-
	// соединений (HTML/CSS/JS/картинки/аналитика), и без явного потолка
	// суммарная память легко улетает за бюджет iOS NE (~50МБ, см.
	// init() выше). 256 КиБ на соединение — компромисс: достаточно для
	// нормального throughput на одно соединение, но не даёт 30+
	// соединениям растащить память бесконтрольно. Min/Default — то, что
	// у gVisor стоит по умолчанию само (не трогаем нижнюю границу).
	// Max 64 КиБ (было 256 КиБ): gVisor авто-тюнит TCP-буфера ВВЕРХ под
	// throughput и НЕ сжимает обратно — при 64 TCP relay × 128 КиБ
	// (send+receive max) = 8 МиБ потолок, вместо прежних 32 МиБ.
	for _, opt := range []tcpip.SettableTransportProtocolOption{
		&tcpip.TCPReceiveBufferSizeRangeOption{Min: 4 << 10, Default: 16 << 10, Max: 64 << 10},
		&tcpip.TCPSendBufferSizeRangeOption{Min: 4 << 10, Default: 16 << 10, Max: 64 << 10},
	} {
		if err := netStack.SetTransportProtocolOption(tcp.ProtocolNumber, opt); err != nil {
			_ = hyClient.Close()
			return nil, fmt.Errorf("netunnel: set tcp buffer option: %s", err)
		}
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

	handle := &TunnelHandle{
		vtun:              vtun,
		stack:             netStack,
		hyConfig:          hyConfig,
		obfsType:          cfg.Obfs.Type,
		obfsPassword:      cfg.Obfs.Salamander.Password,
		client:            hyClient,
		stopMemoryReclaim: make(chan struct{}),
		stopHistory:       make(chan struct{}),
	}
	if cfg.HistoryPath != "" {
		handle.history = &historyRecorder{path: cfg.HistoryPath}
		go handle.recordHistoryPeriodically()
	}

	tcpForwarder := tcp.NewForwarder(netStack, 0, 1024, tcpForwarderHandler(handle))
	netStack.SetTransportProtocolHandler(tcp.ProtocolNumber, tcpForwarder.HandlePacket)
	udpForwarder := udp.NewForwarder(netStack, udpForwarderHandler(handle))
	netStack.SetTransportProtocolHandler(udp.ProtocolNumber, udpForwarder.HandlePacket)

	// Механизмы экономии памяти — только под iOS jetsam; без него они
	// рвали живые соединения (см. Config.MemoryConstrained).
	if cfg.MemoryConstrained {
		go handle.reclaimMemoryPeriodically()
		go handle.evictUnderMemoryPressurePeriodically()
		go handle.reconnectPeriodically()
	}
	return handle, nil
}

// evictUnderMemoryPressurePeriodically — при давлении памяти эвиктит
// все соединения и форсит полный QUIC-реконнект (сбрасывает ВСЁ
// накопленное состояние quic-go). Порог 15 МиБ запаса — достаточно
// рано, чтобы реконнект успел отработать до того, как jetsam убьёт
// процесс. На macOS availableMemoryBytes возвращает "бесконечность"
// (см. memory_other.go), поэтому там это никогда не сработает.
func (h *TunnelHandle) evictUnderMemoryPressurePeriodically() {
	ticker := time.NewTicker(2 * time.Second)
	defer ticker.Stop()
	for {
		select {
		case <-ticker.C:
			if availableMemoryBytes() < 15<<20 {
				h.maybeReconnect()
			}
		case <-h.stopMemoryReclaim:
			return
		}
	}
}

// reconnectPeriodically — основной рычаг против накопления памяти:
// QUIC-соединение (quic-go) трекает ВСЕ streams за свою жизнь и не
// освобождает их state — это архитектурное свойство QUIC, не баг.
// При просмотре Reels за 5 минут создаётся 300-500+ streams, и
// внутренний state quic-go растёт на ~50-100 байт на каждый stream
// навсегда. Единственный способ сбросить — пересоздать клиент.
// 3 минуты — достаточно, чтобы память не дошла до 50 МиБ.
func (h *TunnelHandle) reconnectPeriodically() {
	ticker := time.NewTicker(3 * time.Minute)
	defer ticker.Stop()
	for {
		select {
		case <-ticker.C:
			h.maybeReconnect()
		case <-h.stopMemoryReclaim:
			return
		}
	}
}

// reclaimMemoryPeriodically — SetMemoryLimit/GOGC (см. init() выше)
// заставляют GC запускаться чаще, но не гарантируют, что освобождённые
// страницы реально уйдут обратно ОС прямо сейчас — рантайм сам решает,
// когда звать madvise, и может придерживать память "на будущее". Под
// реальным браузингом (десятки TCP-соединений разом) это и давало
// эффект "память не сбрасывается" — после всплеска нагрузки RSS
// оставался высоким даже когда соединения уже закрылись. FreeOSMemory
// форсирует полный GC + немедленный возврат страниц ОС, не дожидаясь
// рантайм-эвристики. Раз в 10с — компромисс между "RSS реально падает"
// и "не жжём CPU на лишние stop-the-world паузы постоянно".
func (h *TunnelHandle) reclaimMemoryPeriodically() {
	ticker := time.NewTicker(10 * time.Second)
	defer ticker.Stop()
	for {
		select {
		case <-ticker.C:
			debug.FreeOSMemory()
		case <-h.stopMemoryReclaim:
			return
		}
	}
}

// WritePacket — пакет ОТ Swift (NEPacketTunnelFlow.readPackets), отдаём
// в gVisor stack как будто он пришёл из настоящего TUN.
func (h *TunnelHandle) WritePacket(pkt []byte) error {
	atomic.AddUint64(&h.txBytes, uint64(len(pkt)))
	return h.vtun.deliverInbound(pkt)
}

// ReadPacket — блокируется до следующего пакета, который gVisor stack
// хочет отправить К Swift (NEPacketTunnelFlow.writePackets), либо до Stop().
func (h *TunnelHandle) ReadPacket() ([]byte, error) {
	pkt, err := h.vtun.takeOutbound()
	if err == nil {
		atomic.AddUint64(&h.rxBytes, uint64(len(pkt)))
	}
	return pkt, err
}

// GetStats — снимок суммарного трафика с начала жизни хендла, в виде
// JSON-строки (gomobile bind не маршалит произвольные Go-структуры через
// границу с Swift — см. doc-комментарий пакета). Опрашивается из Swift
// по запросу через handleAppMessage, не пушится самостоятельно: PacketTunnelProvider
// не имеет собственного таймера, инициатива опроса — на стороне Rust
// (см. engine/macos.rs::get_traffic_totals_blocking).
func (h *TunnelHandle) GetStats() string {
	tx := atomic.LoadUint64(&h.txBytes)
	rx := atomic.LoadUint64(&h.rxBytes)

	var m runtime.MemStats
	runtime.ReadMemStats(&m)

	connRegistryMu.Lock()
	registrySize := len(connRegistry)
	connRegistryMu.Unlock()

	return fmt.Sprintf(
		`{"txBytes":%d,"rxBytes":%d,"heapInUse":%d,"heapSys":%d,"goroutines":%d,"tcpRelays":%d,"udpRelays":%d,"registrySize":%d,"availMem":%d}`,
		tx, rx,
		m.HeapInuse, m.Sys,
		runtime.NumGoroutine(),
		activeTCPRelays.Load(), activeUDPRelays.Load(),
		registrySize,
		availableMemoryBytes(),
	)
}

func (h *TunnelHandle) Stop() error {
	// сначала дописать хвост трафика в историю — после Stop счётчики
	// этой сессии больше никто не прочитает
	close(h.stopHistory)
	h.FlushHistory()
	close(h.stopMemoryReclaim)
	h.stack.Close()
	_ = h.vtun.Close()
	h.clientMu.RLock()
	c := h.client
	h.clientMu.RUnlock()
	return c.Close()
}
