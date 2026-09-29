#!/bin/bash
# Собирает GoNetunnel.xcframework из packaging/hysteria2-patch/netunnel/ —
# нужно прогнать перед первым открытием Xcode-проекта (и после любых
# изменений в netunnel/*.go). Результат НЕ коммитится (build-артефакт,
# см. .gitignore) — пересобирается на любой машине с Go+gomobile+Xcode.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
OUT_DIR="$SCRIPT_DIR/Frameworks"
TARGETS="${1:-macos,ios}" # передать "macos" или "ios", чтобы собрать только одну платформу

mkdir -p "$OUT_DIR"

# build.sh самого hysteria2-форка уже умеет копировать netunnel/*.go в
# app/netunnel/ — переиспользуем тот же клон, чтобы не дублировать логику
# патча/cp здесь. --skip-cli — нам не нужны linux/darwin CLI-бинарники
# этого скрипта (это отдельный sidecar-артефакт для Linux-сборки), только
# подготовленное дерево для gomobile bind ниже — раньше это съедало
# время на сборку 4 ненужных тут бинарников при каждом запуске.
"$REPO_ROOT/packaging/hysteria2-patch/build.sh" --skip-cli

cd "$REPO_ROOT/packaging/hysteria2-patch/build/hysteria/app"
go get -tool golang.org/x/mobile/cmd/gobind

# Минимальные версии ОС — те же, что deploymentTarget в project.yml
# (macos-ext/ и app/src-tauri/gen/apple/) и minimumSystemVersion в
# tauri.conf.json. Без явного -macosversion gomobile подставляет версию
# macOS САМОЙ машины сборки: собрав на macOS 27, линкер расширения
# (таргет 13.3) выдал «object file was built for newer 'macOS' version
# (27.0) than being linked (13.3)» — такой .appex мог не загрузиться у
# пользователей на более старой macOS. -iosversion по умолчанию 13.0 —
# задаём явно для симметрии.
MACOS_MIN_VERSION="13.3"
IOS_MIN_VERSION="16.4"

rm -rf "$OUT_DIR/GoNetunnel.xcframework"
gomobile bind -target "$TARGETS" \
    -macosversion "$MACOS_MIN_VERSION" -iosversion "$IOS_MIN_VERSION" \
    -o "$OUT_DIR/GoNetunnel.xcframework" ./netunnel

echo "✓ $OUT_DIR/GoNetunnel.xcframework собран"
