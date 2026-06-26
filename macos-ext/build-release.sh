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
# полного .app.
#
# Раньше здесь был голый `hdiutil create -srcfolder` — он кладёт .app
# в образ БЕЗ ярлыка на /Applications и без стандартного окна
# "перетащи установить". Внешне это выглядит как просто папка с
# приложением — непонятно, что с ней делать, и драг-инсталл не
# получается сам собой. Используем тот же bundle_dmg.sh, которым Tauri
# сам собрал DMG на шаге [3/4] (он уже лежит в DMG_DIR после первого
# запуска) — с `--app-drop-link`, как делают все нормальные macOS-инсталляторы.
BUNDLE_DMG="$DMG_DIR/bundle_dmg.sh"
STAGING_DIR="$(mktemp -d)"
trap 'rm -rf "$STAGING_DIR"' EXIT
# bundle_dmg.sh копирует СОДЕРЖИМОЕ source_folder внутрь образа — если
# отдать ему сам .app, в образе окажется распакованное "Contents/" вместо
# одного .app-файла. Поэтому .app кладём в отдельную пустую папку и её
# отдаём как source_folder (тот же приём использует сам Tauri).
ditto "$APP_PATH" "$STAGING_DIR/vrox.vpn.app"

rm -f "$DMG_DIR/$DMG_NAME"
"$BUNDLE_DMG" \
    --volname "vrox.vpn" \
    --window-size 540 380 \
    --icon-size 128 \
    --icon "vrox.vpn.app" 140 170 \
    --app-drop-link 400 170 \
    --hide-extension "vrox.vpn.app" \
    "$DMG_DIR/$DMG_NAME" "$STAGING_DIR"

echo ""
echo "✓ Готово (этот DMG — только для прямого/локального теста, не для"
echo "  раздачи: основной канал доставки macOS теперь TestFlight, см."
echo "  macos-ext/build-testflight.sh):"
echo "  $APP_PATH"
echo "  $DMG_DIR/$DMG_NAME"
