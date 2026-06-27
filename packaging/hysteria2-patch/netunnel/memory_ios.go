//go:build ios

package netunnel

/*
#include <os/proc.h>
*/
import "C"

// availableMemoryBytes — сколько байт осталось до текущего лимита
// "dirty memory" процесса, по данным самой iOS (НЕ наша догадка про
// потолок ~50МБ). API_UNAVAILABLE(macos) — отсюда build tag "ios": на
// macOS этого символа нет даже в линкуемой библиотеке, хотя он
// присутствует в заголовке SDK для документации.
func availableMemoryBytes() uint64 {
	return uint64(C.os_proc_available_memory())
}
