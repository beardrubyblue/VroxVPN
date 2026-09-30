package netunnel

import (
	"bytes"
	"testing"
)

// Регрессия: gomobile передаёт []byte по ссылке на память Swift, которую
// после вызова переиспользуют. wgTun обязан скопировать пакет в
// deliverInbound — иначе WireGuard шифрует уже перезаписанные байты
// (на iPhone: «подключено, но сети нет»).
func TestWGTunCopiesInboundPacket(t *testing.T) {
	tunnel := newWGTun(defaultWireGuardMTU)
	defer tunnel.Close()

	packet := []byte{0x45, 0x00, 0x00, 0x1c, 1, 2, 3, 4}
	want := append([]byte(nil), packet...)
	if err := tunnel.deliverInbound(packet); err != nil {
		t.Fatal(err)
	}
	for i := range packet { // Swift освободил и переиспользовал память
		packet[i] = 0xff
	}

	const offset = 16
	bufs := [][]byte{make([]byte, offset+defaultWireGuardMTU)}
	sizes := make([]int, 1)
	if _, err := tunnel.Read(bufs, sizes, offset); err != nil {
		t.Fatal(err)
	}
	if got := bufs[0][offset : offset+sizes[0]]; !bytes.Equal(got, want) {
		t.Fatalf("got %x, want %x", got, want)
	}
}
