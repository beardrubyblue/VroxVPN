package netunnel

import (
	"encoding/binary"
	"net/netip"
	"os"
	"testing"
	"time"
)

// Живая проверка WireGuard на настоящем сервере: поднять тоннель, отправить
// через него DNS-запрос к 1.1.1.1 и дождаться ответа. Запускается, только
// если VROX_WG_TEST_CONFIG указывает на JSON-конфиг (формат WireGuardConfig)
// и VROX_WG_TEST_ADDRESS — адрес клиента в тоннеле (Address из .conf, без
// маски). Конфиг с ключами в репозиторий не кладётся.
func TestWireGuardLiveDNS(t *testing.T) {
	configPath, address := os.Getenv("VROX_WG_TEST_CONFIG"), os.Getenv("VROX_WG_TEST_ADDRESS")
	if configPath == "" || address == "" {
		t.Skip("VROX_WG_TEST_CONFIG / VROX_WG_TEST_ADDRESS не заданы")
	}
	configJSON, err := os.ReadFile(configPath)
	if err != nil {
		t.Fatal(err)
	}
	tunnel, err := Start(string(configJSON))
	if err != nil {
		t.Fatal(err)
	}
	defer tunnel.Stop()

	query := dnsQueryPacket(t, netip.MustParseAddr(address), netip.MustParseAddr("1.1.1.1"))
	replies := make(chan []byte, 1)
	go func() {
		for {
			pkt, err := tunnel.ReadPacket()
			if err != nil {
				return
			}
			if len(pkt) > 20 && pkt[9] == 17 && netip.AddrFrom4([4]byte(pkt[12:16])).String() == "1.1.1.1" {
				replies <- pkt
				return
			}
		}
	}()

	deadline := time.After(15 * time.Second)
	for {
		if err := tunnel.WritePacket(query); err != nil {
			t.Fatal(err)
		}
		select {
		case reply := <-replies:
			t.Logf("DNS-ответ через тоннель: %d байт; stats: %s", len(reply), tunnel.GetStats())
			return
		case <-time.After(2 * time.Second): // рукопожатие ещё идёт — повторить
		case <-deadline:
			t.Fatalf("нет ответа за 15с; stats: %s", tunnel.GetStats())
		}
	}
}

// dnsQueryPacket — IPv4/UDP-пакет с DNS-запросом A example.com.
func dnsQueryPacket(t *testing.T, source, destination netip.Addr) []byte {
	t.Helper()
	dns := []byte{0x12, 0x34, 0x01, 0x00, 0, 1, 0, 0, 0, 0, 0, 0}
	dns = append(dns, 7, 'e', 'x', 'a', 'm', 'p', 'l', 'e', 3, 'c', 'o', 'm', 0, 0, 1, 0, 1)

	udp := make([]byte, 8, 8+len(dns))
	binary.BigEndian.PutUint16(udp[0:], 53535)
	binary.BigEndian.PutUint16(udp[2:], 53)
	binary.BigEndian.PutUint16(udp[4:], uint16(8+len(dns)))
	udp = append(udp, dns...) // контрольная сумма UDP 0 — допустимо для IPv4

	ip := make([]byte, 20, 20+len(udp))
	ip[0], ip[8], ip[9] = 0x45, 64, 17
	binary.BigEndian.PutUint16(ip[2:], uint16(20+len(udp)))
	copy(ip[12:16], source.AsSlice())
	copy(ip[16:20], destination.AsSlice())
	var sum uint32
	for i := 0; i < 20; i += 2 {
		sum += uint32(binary.BigEndian.Uint16(ip[i:]))
	}
	for sum > 0xffff {
		sum = sum&0xffff + sum>>16
	}
	binary.BigEndian.PutUint16(ip[10:], ^uint16(sum))
	return append(ip, udp...)
}
