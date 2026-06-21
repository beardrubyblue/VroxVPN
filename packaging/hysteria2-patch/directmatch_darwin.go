//go:build darwin

package tun

import (
	"net"
	"syscall"

	"golang.org/x/sys/unix"
)

// dialDirect открывает соединение мимо тоннеля. На macOS нет SO_BINDTODEVICE
// (это Linux-специфичная опция) — тот же эффект (привязка исходящего сокета
// к конкретному физическому интерфейсу) даёт IP_BOUND_IF, см. также
// docs/ARCHITECTURE.md "macOS: IP_BOUND_IF".
func (m *directMatcher) dialDirect(network, address string) (net.Conn, error) {
	iface, err := net.InterfaceByName(m.iface)
	if err != nil {
		return nil, err
	}
	dialer := net.Dialer{
		Control: func(_, _ string, c syscall.RawConn) error {
			var setErr error
			ctrlErr := c.Control(func(fd uintptr) {
				setErr = unix.SetsockoptInt(int(fd), unix.IPPROTO_IP, unix.IP_BOUND_IF, iface.Index)
			})
			if ctrlErr != nil {
				return ctrlErr
			}
			return setErr
		},
	}
	return dialer.Dial(network, address)
}
