//go:build darwin

package tun

import (
	"errors"

	"go.uber.org/zap"
)

// startDNSSniffer и defaultInterfaceName на macOS не реализованы: Linux-
// версия использует raw AF_PACKET socket и /proc/net/route — ни того, ни
// другого на macOS нет. Аналог потребовал бы захвата трафика через
// BPF-устройство (/dev/bpf) и парсинга route-сокета — отдельная задача, не
// делалась. Пока directDomains на macOS отключена: defaultInterfaceName
// всегда возвращает ошибку, поэтому matcher.enabled() == false и весь
// трафик идёт через тоннель как обычно (безопасный fallback, не утечка
// мимо VPN, см. вызов в server.go: ошибка просто логируется как warning).
func startDNSSniffer(matcher *directMatcher, logger *zap.Logger, ifaceName string) {}

func defaultInterfaceName() (string, error) {
	return "", errors.New("directDomains: sniffer не реализован на macOS")
}
