#!/bin/bash
# Единая команда полной macOS-сборки: Go-фреймворк → Xcode (.appex,
# Release) → Tauri (.app, release) → встраивание .appex → пересборка
# DMG. Заменяет ручную последовательность из нескольких скриптов,
# которая раньше требовалась после каждого `pnpm tauri build` (см.
# git log/docs/ARCHITECTURE.md — DMG из голого `pnpm tauri build`
# никогда не содержал .appex, потому что Tauri бандлит DMG из .app ДО
# того, как .appex туда кто-либо положит, а делать это руками каждый
# раз — единственная причина, по которой DMG раньше был "битым").
#
# Результат: app/src-tauri/target/release/bundle/macos/vrox.vpn.app и
# одноимённый .dmg в той же папке — уже с встроенным и подписанным
# .appex, готовые к раздаче без дополнительных шагов.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
APP_PATH="$REPO_ROOT/app/src-tauri/target/release/bundle/macos/vrox.vpn.app"
DMG_DIR="$REPO_ROOT/app/src-tauri/target/release/bundle/dmg"
DMG_NAME="vrox.vpn_$(node -p "require('$REPO_ROOT/app/src-tauri/tauri.conf.json').version" 2>/dev/null || echo 4.0.0)_aarch64.dmg"

echo "==> [1/4] Go-фреймворк (GoNetunnel.xcframework)"
"$SCRIPT_DIR/build-go-framework.sh" macos

echo "==> [2/4] Xcode .appex (Release)"
xcodebuild -project "$SCRIPT_DIR/VroxVPNNetworkExtension.xcodeproj" \
    -scheme VroxVPNHost -configuration Release -allowProvisioningUpdates build \
    | tail -5

echo "==> [3/4] Tauri .app (release)"
(cd "$REPO_ROOT/app" && pnpm tauri build)

echo "==> [4/4] Встраивание .appex + пересборка DMG"
"$SCRIPT_DIR/embed-into-tauri-app.sh" "$APP_PATH" Release

# Tauri уже создал DMG на шаге [3/4] — но БЕЗ .appex (бандлинг DMG
# случился раньше встраивания на шаге [4/4]). Пересобираем DMG из уже
# полного .app, а не патчим старый образ — hdiutil проще и надёжнее,
# чем редактировать существующий .dmg.
rm -f "$DMG_DIR/$DMG_NAME"
hdiutil create -volname "vrox.vpn" -srcfolder "$APP_PATH" -ov -format UDZO "$DMG_DIR/$DMG_NAME"

echo ""
echo "✓ Готово:"
echo "  $APP_PATH"
echo "  $DMG_DIR/$DMG_NAME"
