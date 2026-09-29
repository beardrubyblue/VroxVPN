//go:build !ios

package netunnel

import "math"

// availableMemoryBytes — на macOS NE такого жёсткого потолка нет (см.
// API_UNAVAILABLE(macos) у os_proc_available_memory, memory_ios.go) —
// возвращаем "практически бесконечность", чтобы эвикшен-логика,
// завязанная на этот сигнал, на macOS никогда не срабатывала.
// isMemoryConstrained — false на macOS: у NE-расширения нет iOS-шного
// jetsam-потолка. Раньше iOS-механизмы экономии памяти работали и здесь,
// и ломали долгоживущие соединения: reconnectPeriodically каждые 3 мин
// пересоздавал QUIC-клиент и эвиктил ВСЕ relay-соединения — приложение
// Claude (длинное соединение) уходило в бесконечный реконнект, в
// браузере сыпались ERR_CONNECTION_CLOSED. Плюс GOMAXPROCS(1) зря
// ограничивал пропускную способность на многоядерном Mac.
const isMemoryConstrained = false

func availableMemoryBytes() uint64 {
	return math.MaxUint64
}
