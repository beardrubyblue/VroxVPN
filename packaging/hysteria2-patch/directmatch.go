package tun

import (
	"net"
	"strings"
	"sync"
	"syscall"
	"time"

	"github.com/miekg/dns"
	"golang.org/x/sys/unix"
)

// directMatcher решает, какие домены должны идти мимо тоннеля (direct), и
// запоминает domain->IP, подсматривая за DNS-ответами, которые и так летят
// через этот же TUN-хендлер (как обычный UDP-трафик) — без внешнего
// DNS-сервера и без изменений в системной таблице маршрутизации.
type directMatcher struct {
	suffixes map[string]struct{} // домены в нижнем регистре, без точки на конце
	iface    string              // настоящий интерфейс — нужен, чтобы наш
	// собственный исходящий dial не попал в тот же TUN заново (см. dialer())

	mu        sync.Mutex
	directIPs map[string]time.Time // ip -> время истечения (по DNS TTL)
}

func newDirectMatcher(domains []string, iface string) *directMatcher {
	m := &directMatcher{
		suffixes:  make(map[string]struct{}, len(domains)),
		directIPs: make(map[string]time.Time),
		iface:     iface,
	}
	for _, d := range domains {
		d = strings.ToLower(strings.TrimSuffix(strings.TrimSpace(d), "."))
		if d != "" {
			m.suffixes[d] = struct{}{}
		}
	}
	return m
}

// dialDirect открывает соединение мимо тоннеля. SO_BINDTODEVICE обязателен:
// без него новый сокет снова попадёт под политику маршрутизации TUN (тот же
// самый default route 0.0.0.0/0 -> tun-vroxory) и зациклится сам на себя —
// настоящий тоннель этой проблемы не имеет только потому, что его сокет к
// VPN-серверу создаётся ДО того, как TUN включает свои правила.
func (m *directMatcher) dialDirect(network, address string) (net.Conn, error) {
	dialer := net.Dialer{
		Control: func(_, _ string, c syscall.RawConn) error {
			var bindErr error
			ctrlErr := c.Control(func(fd uintptr) {
				bindErr = unix.BindToDevice(int(fd), m.iface)
			})
			if ctrlErr != nil {
				return ctrlErr
			}
			return bindErr
		},
	}
	return dialer.Dial(network, address)
}

func (m *directMatcher) enabled() bool {
	return len(m.suffixes) > 0 && m.iface != ""
}

// matchesDomain — суффиксное сопоставление: example.com в списке matchит
// и example.com, и foo.example.com.
func (m *directMatcher) matchesDomain(name string) bool {
	name = strings.ToLower(strings.TrimSuffix(name, "."))
	for {
		if _, ok := m.suffixes[name]; ok {
			return true
		}
		idx := strings.IndexByte(name, '.')
		if idx == -1 {
			return false
		}
		name = name[idx+1:]
	}
}

// observeDNSResponse разбирает сырой DNS-ответ; если вопрос совпал с нашим
// списком доменов, запоминает все IP из ответа как "direct" до истечения TTL.
func (m *directMatcher) observeDNSResponse(payload []byte) {
	if !m.enabled() {
		return
	}
	var msg dns.Msg
	if err := msg.Unpack(payload); err != nil {
		return
	}
	if len(msg.Question) == 0 || !m.matchesDomain(msg.Question[0].Name) {
		return
	}
	now := time.Now()
	m.mu.Lock()
	defer m.mu.Unlock()
	for _, rr := range msg.Answer {
		var ip net.IP
		var ttl uint32
		switch rec := rr.(type) {
		case *dns.A:
			ip, ttl = rec.A, rec.Hdr.Ttl
		case *dns.AAAA:
			ip, ttl = rec.AAAA, rec.Hdr.Ttl
		default:
			continue
		}
		if ttl < 60 {
			ttl = 60 // не даём записи протухнуть мгновенно при низком/нулевом TTL
		}
		m.directIPs[ip.String()] = now.Add(time.Duration(ttl) * time.Second)
	}
}

// isDirectIP сообщает, нужно ли пускать трафик на этот IP мимо тоннеля —
// на основе того, что мы выучили из DNS-ответов.
func (m *directMatcher) isDirectIP(ip string) bool {
	if !m.enabled() {
		return false
	}
	m.mu.Lock()
	defer m.mu.Unlock()
	expiry, ok := m.directIPs[ip]
	if !ok {
		return false
	}
	if time.Now().After(expiry) {
		delete(m.directIPs, ip)
		return false
	}
	return true
}
