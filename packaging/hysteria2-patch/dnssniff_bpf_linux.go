//go:build linux

package tun

import "golang.org/x/sys/unix"

// attachDNSResponseFilter вешает на сокет классический BPF, пропускающий
// только IPv4/UDP с порта 53 (= `tcpdump -dd 'ip and udp src port 53'`).
//
// Раньше фильтра не было, и AF_PACKET-сокет получал ВСЕ пакеты ens3 — по
// syscall'у и копии на пакет. Поймано на шлюзе vroxgw: после снятия
// потолка bandwidth vroxcore ел 197% CPU (3 vCPU) на ~140 Мбит/с, загрузка
// падала до 26 Мбит/с; на 1 vCPU — 88. Трафик LAN-клиентов проходит через
// ens3 дважды (QUIC к серверу + пересылка клиенту), так что сниффер
// разбирал в userspace весь тоннель. Фильтр отсекает это в ядре.
func attachDNSResponseFilter(fd int) error {
	filter := dnsResponseFilter()
	prog := unix.SockFprog{Len: uint16(len(filter)), Filter: &filter[0]}
	return unix.SetsockoptSockFprog(fd, unix.SOL_SOCKET, unix.SO_ATTACH_FILTER, &prog)
}

// dnsResponseFilter — сама программа; отдельно, чтобы её можно было
// прогнать в bpf.VM без настоящего сокета.
func dnsResponseFilter() []unix.SockFilter {
	const (
		dropJumpFromIP    = 8 // до "ret 0" от инструкции 1
		dropJumpFromProto = 6 // от инструкции 3
		dropJumpFromFrag  = 4 // от инструкции 5 (jt)
		acceptSnapLen     = 0x40000
	)
	return []unix.SockFilter{
		{Code: 0x28, K: 12},                           // ldh [12] — EtherType
		{Code: 0x15, Jf: dropJumpFromIP, K: 0x0800},   // IPv4?
		{Code: 0x30, K: 23},                           // ldb [23] — IP protocol
		{Code: 0x15, Jf: dropJumpFromProto, K: 17},    // UDP?
		{Code: 0x28, K: 20},                           // ldh [20] — flags/fragment offset
		{Code: 0x45, Jt: dropJumpFromFrag, K: 0x1fff}, // не первый фрагмент — мимо
		{Code: 0xb1, K: 14},                           // ldxb 4*([14]&0xf) — длина IP-заголовка
		{Code: 0x48, K: 14},                           // ldh [x+14] — UDP src port
		{Code: 0x15, Jf: 1, K: 53},                    // == 53?
		{Code: 0x06, K: acceptSnapLen},                // ret — принять
		{Code: 0x06, K: 0},                            // ret 0 — отбросить
	}
}
