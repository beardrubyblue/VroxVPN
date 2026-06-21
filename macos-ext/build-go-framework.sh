#!/bin/bash
# Собирает GoNetunnel.xcframework из packaging/hysteria2-patch/netunnel/ —
# нужно прогнать перед первым открытием Xcode-проекта (и после любых
# изменений в netunnel/*.go). Результат НЕ коммитится (build-артефакт,
# см. .gitignore) — пересобирается на любой машине с Go+gomobile+Xcode.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
OUT_DIR="$SCRIPT_DIR/Frameworks"
TARGETS="${1:-macos}" # передать "macos,ios" для обеих платформ сразу

mkdir -p "$OUT_DIR"

# build.sh самого hysteria2-форка уже умеет копировать netunnel/*.go в
# app/netunnel/ — переиспользуем тот же клон, чтобы не дублировать логику
# патча/cp здесь
"$REPO_ROOT/packaging/hysteria2-patch/build.sh"

cd "$REPO_ROOT/packaging/hysteria2-patch/build/hysteria/app"
go get -tool golang.org/x/mobile/cmd/gobind

rm -rf "$OUT_DIR/GoNetunnel.xcframework"
gomobile bind -target "$TARGETS" -o "$OUT_DIR/GoNetunnel.xcframework" ./netunnel

echo "✓ $OUT_DIR/GoNetunnel.xcframework собран"
