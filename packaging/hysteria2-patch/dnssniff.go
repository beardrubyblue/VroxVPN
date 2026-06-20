package tun

import (
	"bufio"
	"encoding/binary"
	"fmt"
	"net"
	"os"
	"strings"
	"syscall"

	"go.uber.org/zap"
)

// startDNSSniffer пассивно слушает порт 53 на НАСТОЯЩЕМ сетевом интерфейсе
// (не на TUN) — потому что sing-tun сам исключает DNS-трафик из тоннеля
// политикой маршрутизации (см. apernet/sing-tun tun_linux.go, правило
// "not dport 53"), и наш TUN-хендлер его никогда не видит. DNS и так идёт
// в открытом виде мимо тоннеля — мы только подсматриваем, ничего не меняя.
func startDNSSniffer(matcher *directMatcher, logger *zap.Logger, ifaceName string) {
	if !matcher.enabled() {
		return
	}
	iface, err := net.InterfaceByName(ifaceName)
	if err != nil {
		logger.Warn("dns-sniffer: InterfaceByName", zap.Error(err))
		return
	}

	fd, err := syscall.Socket(syscall.AF_PACKET, syscall.SOCK_RAW, int(htons(syscall.ETH_P_IP)))
	if err != nil {
		logger.Warn("dns-sniffer: socket", zap.Error(err))
		return
	}
	addr := syscall.SockaddrLinklayer{
		Protocol: htons(syscall.ETH_P_IP),
		Ifindex:  iface.Index,
	}
	if err := syscall.Bind(fd, &addr); err != nil {
		logger.Warn("dns-sniffer: bind", zap.Error(err))
		_ = syscall.Close(fd)
		return
	}

	logger.Info("DNS sniffer запущен", zap.String("interface", ifaceName))

	go func() {
		defer syscall.Close(fd)
		buf := make([]byte, 65536)
		for {
			n, _, recvErr := syscall.Recvfrom(fd, buf, 0)
			if recvErr != nil {
				logger.Debug("dns-sniffer: recvfrom завершился", zap.Error(recvErr))
				return
			}
			extractDNSPayload(buf[:n], matcher)
		}
	}()
}

func htons(i uint16) uint16 {
	return i<<8 | i>>8
}

// extractDNSPayload разбирает Ethernet+IPv4+UDP вручную (без libpcap) и,
// если это ответ с порта 53, передаёт DNS-сообщение в matcher.
func extractDNSPayload(frame []byte, matcher *directMatcher) {
	const ethHeaderLen = 14
	if len(frame) < ethHeaderLen+20+8 {
		return
	}
	if binary.BigEndian.Uint16(frame[12:14]) != 0x0800 { // только IPv4
		return
	}
	ipHeader := frame[ethHeaderLen:]
	if (ipHeader[0] >> 4) != 4 { // version
		return
	}
	ihl := int(ipHeader[0]&0x0F) * 4
	if ihl < 20 || len(ipHeader) < ihl+8 {
		return
	}
	if ipHeader[9] != 17 { // protocol == UDP
		return
	}
	udpHeader := ipHeader[ihl:]
	srcPort := binary.BigEndian.Uint16(udpHeader[0:2])
	if srcPort != 53 {
		return
	}
	udpLen := int(binary.BigEndian.Uint16(udpHeader[4:6]))
	if udpLen < 8 || len(udpHeader) < 8 {
		return
	}
	end := udpLen
	if end > len(udpHeader) {
		end = len(udpHeader)
	}
	payload := udpHeader[8:end]
	if len(payload) > 0 {
		matcher.observeDNSResponse(payload)
	}
}

// defaultInterfaceName ищет интерфейс с дефолтным маршрутом в основной
// таблице (254/main) — её hysteria2/sing-tun не трогает, только добавляет
// правила политики маршрутизации поверх, так что main всегда хранит
// настоящий шлюз, даже когда TUN активен.
func defaultInterfaceName() (string, error) {
	f, err := os.Open("/proc/net/route")
	if err != nil {
		return "", err
	}
	defer f.Close()

	scanner := bufio.NewScanner(f)
	scanner.Scan() // заголовок
	for scanner.Scan() {
		fields := strings.Fields(scanner.Text())
		if len(fields) < 8 {
			continue
		}
		iface, dest, _, _, _, _, _, mask := fields[0], fields[1], fields[2], fields[3], fields[4], fields[5], fields[6], fields[7]
		if dest == "00000000" && mask == "00000000" {
			return iface, nil
		}
	}
	return "", fmt.Errorf("default route not found in /proc/net/route")
}
