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
// isMemoryConstrained — iOS убивает NE-расширение по jetsam на ~50МБ,
// поэтому здесь включены GOMAXPROCS(1)/SetMemoryLimit (init() в
// netunnel.go) и фоновые механизмы экономии памяти (периодический
// QUIC-реконнект, эвикшен, FreeOSMemory). На macOS — false, см.
// memory_other.go.
const isMemoryConstrained = true

func availableMemoryBytes() uint64 {
	return uint64(C.os_proc_available_memory())
}
