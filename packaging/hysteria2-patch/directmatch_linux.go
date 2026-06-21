//go:build linux

package tun

import (
	"net"
	"syscall"

	"golang.org/x/sys/unix"
)

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
