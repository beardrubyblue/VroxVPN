#!/bin/bash
# Собирает netunnel (hysteria2 + WireGuard/AmneziaWG, тот же Go-код, что
# у iOS — packaging/hysteria2-patch/netunnel) в .aar для Android через
# gomobile bind. Результат — в app/src-tauri/gen/android/app/libs/
# (build-артефакт, не коммитится, см. .gitignore).
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
source "$SCRIPT_DIR/env.sh"
OUT_DIR="$REPO_ROOT/app/src-tauri/gen/android/app/libs"

# Минимальный Android — 12 (API 31): тот же minSdkVersion, что в
# tauri.android.conf.json. Обход RU и локальной сети (excludeRoute) — только
# с Android 13, см. VroxVpnService.kt.
ANDROID_API=31

# Подготовленное дерево форка (клон + патч + netunnel/) — как у iOS.
"$REPO_ROOT/packaging/hysteria2-patch/build.sh" --skip-cli

cd "$REPO_ROOT/packaging/hysteria2-patch/build/hysteria/app"
go get -tool golang.org/x/mobile/cmd/gobind

mkdir -p "$OUT_DIR"
# Только arm64: Android 13+ — это arm64-телефоны, а каждая лишняя ABI
# (arm, 386, amd64) — ещё ~40 МБ Go-рантайма с gVisor в APK.
gomobile bind -target android/arm64 -androidapi "$ANDROID_API" \
    -javapkg com.vroxory \
    -o "$OUT_DIR/netunnel.aar" ./netunnel

echo "✓ $OUT_DIR/netunnel.aar собран"
