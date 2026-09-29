//go:build !ios

package netunnel

import "math"

// availableMemoryBytes — заглушка для не-iOS сборки (go vet на darwin-
// хосте в build.sh): os_proc_available_memory — API_UNAVAILABLE(macos),
// см. memory_ios.go. "Практически бесконечность" — эвикшен не сработает.
func availableMemoryBytes() uint64 {
	return math.MaxUint64
}
