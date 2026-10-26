package netunnel

import (
	"os"
	"syscall"
)

// maxPacketSize — буфер чтения из TUN: больше любого MTU, который мы ставим.
const maxPacketSize = 65535

// fdTunnel — тоннель, сам качающий пакеты из/в TUN-дескриптор. Для Android:
// VpnService отдаёт fd, и гонять каждый пакет через JNI (как Swift через
// packetFlow на iOS) — лишний переход и аллокация byte[] на пакет в Kotlin.
// Остальные методы (GetStats, Stop…) — от вложенного протокольного тоннеля.
type fdTunnel struct {
	Tunnel
	file *os.File
}

// StartFD — вход для Android (VpnService): как Start, но пакеты читаются и
// пишутся в fd напрямую. fd переходит во владение Go (Kotlin отдаёт его
// через ParcelFileDescriptor.detachFd) и закрывается в Stop.
func StartFD(configJSON string, fd int) (Tunnel, error) {
	inner, err := Start(configJSON)
	if err != nil {
		_ = syscall.Close(fd)
		return nil, err
	}
	// Неблокирующий fd попадает в poller рантайма — тогда Close в Stop
	// прерывает висящий Read. С блокирующим (VpnService отдаёт именно
	// такой) горутина чтения осталась бы висеть в read(2) навсегда.
	if err := syscall.SetNonblock(fd, true); err != nil {
		_ = inner.Stop()
		_ = syscall.Close(fd)
		return nil, err
	}
	tunnel := &fdTunnel{Tunnel: inner, file: os.NewFile(uintptr(fd), "vrox-tun")}
	go tunnel.pumpFromOS()
	go tunnel.pumpToOS()
	return tunnel, nil
}

// pumpFromOS — пакеты приложений из TUN в тоннель. Буфер один на всё время:
// WritePacket обоих протоколов копирует пакет до возврата (virtual_tun.go,
// wg_tun.go::deliverInbound).
func (t *fdTunnel) pumpFromOS() {
	buf := make([]byte, maxPacketSize)
	for {
		n, err := t.file.Read(buf)
		if err != nil {
			return // Stop закрыл fd
		}
		_ = t.Tunnel.WritePacket(buf[:n]) // не-IP пакет просто отбрасываем
	}
}

// pumpToOS — ответы из тоннеля обратно в TUN.
func (t *fdTunnel) pumpToOS() {
	for {
		pkt, err := t.Tunnel.ReadPacket()
		if err != nil {
			return // Stop закрыл тоннель
		}
		if _, err := t.file.Write(pkt); err != nil && err == os.ErrClosed {
			return
		}
	}
}

func (t *fdTunnel) Stop() error {
	_ = t.file.Close()
	return t.Tunnel.Stop()
}
