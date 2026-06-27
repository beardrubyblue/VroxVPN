//go:build !ios

package netunnel

import "math"

// availableMemoryBytes — на macOS NE такого жёсткого потолка нет (см.
// API_UNAVAILABLE(macos) у os_proc_available_memory, memory_ios.go) —
// возвращаем "практически бесконечность", чтобы эвикшен-логика,
// завязанная на этот сигнал, на macOS никогда не срабатывала.
func availableMemoryBytes() uint64 {
	return math.MaxUint64
}
