#!/bin/bash
# Сборка настоящего iOS-приложения (Tauri, app/src-tauri/gen/apple/) для
# TestFlight: Go-фреймворк → `tauri ios build --archive-only` → ручной
# export (.ipa) → инструкция по загрузке через `xcrun altool`.
#
# Раньше этот скрипт собирал VroxVPNHost-iOS из macos-ext/ — голый
# SwiftUI-харнесс со спайка NE (две кнопки, тестовый конфиг 127.0.0.1:1)
# с ТЕМ ЖЕ bundle id com.vroxory.vpn: App Store Connect его принимал, и
# тестировщики получили бы заглушку вместо VPN-клиента. Переписан на
# Tauri-сборку (см. docs/ARCHITECTURE.md, «iOS: настоящее Tauri-приложение»).
#
# Почему export вручную, а не `tauri ios build --export-method`: Tauri
# генерирует ExportOptions с Automatic-подписью, игнорируя Manual-конфиг
# проекта, и падает на «requires a provisioning profile with the Network
# Extensions feature». Поэтому --archive-only + xcodebuild -exportArchive
# с gen/apple/ExportOptionsManual.plist.
#
# Почему бэкап Info.plist/pbxproj: `--build-number` внутри вызывает
# `agvtool new-version -all` (cargo-mobile2), который переписывает
# CFBundleVersion во ВСЕХ Info.plist проекта и CURRENT_PROJECT_VERSION в
# pbxproj — в т.ч. заменяет `$(CURRENT_PROJECT_VERSION)` в Info.plist
# расширения на литерал. Это и была регрессия «CFBundleVersion
# перезатёрся хардкодом», которую уже дважды чинили руками. Файлы
# возвращаются в исходное состояние после сборки (trap на EXIT).
#
# Предпосылки:
#   1. Профили «vrox.vpn iOS App Store» / «vrox.vpn tunnel iOS App Store»
#      (App Store Connect, iOS, capability Network Extensions) и
#      сертификат «Apple Distribution» установлены локально.
#   2. `pnpm install` в app/, Rust-таргет aarch64-apple-ios, gomobile.
#   3. App-specific password для загрузки (appleid.apple.com).
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
APP_DIR="$REPO_ROOT/app"
APPLE_DIR="$APP_DIR/src-tauri/gen/apple"
ARCHIVE_PATH="$APPLE_DIR/build/app_iOS.xcarchive"
EXPORT_PATH="$APPLE_DIR/build/export-appstore"
TEAM_ID="${TEAM_ID:-QRZT5R3Q28}"

# Файлы, которые трогает agvtool (см. шапку) — пути от корня репозитория.
VERSIONED_FILES=(
    "app/src-tauri/gen/apple/app.xcodeproj/project.pbxproj"
    "app/src-tauri/gen/apple/app_iOS/Info.plist"
    "macos-ext/VroxTunnelExtension-iOS/Info.plist"
)

# Монотонный build-номер без ручного счётчика — тот же приём, что в
# build-testflight.sh. Tauri допишет его к версии: 4.0.0 → 4.0.0.<N>.
BUILD_NUMBER="$(git -C "$REPO_ROOT" rev-list --count HEAD)"
echo "→ build-номер: $BUILD_NUMBER"

BACKUP_DIR="$(mktemp -d)"
restore_versioned_files() {
    for file in "${VERSIONED_FILES[@]}"; do
        cp "$BACKUP_DIR/$(basename "$(dirname "$file")")-$(basename "$file")" "$REPO_ROOT/$file"
    done
    rm -rf "$BACKUP_DIR"
}
for file in "${VERSIONED_FILES[@]}"; do
    cp "$REPO_ROOT/$file" "$BACKUP_DIR/$(basename "$(dirname "$file")")-$(basename "$file")"
done
trap restore_versioned_files EXIT

echo "==> [1/4] Go-фреймворк (GoNetunnel.xcframework, слайс ios)"
"$SCRIPT_DIR/build-go-framework.sh" ios

rm -rf "$ARCHIVE_PATH" "$EXPORT_PATH"

echo "==> [2/4] Archive (tauri ios build, Release — Apple Distribution)"
(cd "$APP_DIR" && APPLE_DEVELOPMENT_TEAM="$TEAM_ID" \
    pnpm tauri ios build --archive-only --build-number "$BUILD_NUMBER")

echo "==> [3/4] Export (.ipa, Manual-подпись)"
xcodebuild -exportArchive \
    -archivePath "$ARCHIVE_PATH" \
    -exportPath "$EXPORT_PATH" \
    -exportOptionsPlist "$APPLE_DIR/ExportOptionsManual.plist" | tail -10

IPA_PATH="$(find "$EXPORT_PATH" -maxdepth 1 -name '*.ipa' | head -1)"
if [[ -z "$IPA_PATH" ]]; then
    echo "✗ .ipa не найден в $EXPORT_PATH — шаг export не прошёл?" >&2
    exit 1
fi

# App Store Connect требует одинаковый CFBundleVersion у приложения и
# встроенного расширения — проверяем по архиву, а не верим на слово.
APP_BUNDLE="$ARCHIVE_PATH/Products/Applications/vrox.vpn.app"
APP_VERSION="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleVersion' "$APP_BUNDLE/Info.plist")"
EXT_VERSION="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleVersion' \
    "$APP_BUNDLE/PlugIns/VroxTunnelExtension.appex/Info.plist")"
if [[ "$APP_VERSION" != "$EXT_VERSION" ]]; then
    echo "✗ CFBundleVersion не совпадает: app=$APP_VERSION, extension=$EXT_VERSION" >&2
    exit 1
fi

echo ""
echo "✓ Готово: $IPA_PATH (CFBundleVersion $APP_VERSION)"
echo ""
echo "==> [4/4] Загрузка в App Store Connect:"
echo "  xcrun altool --upload-app -f \"$IPA_PATH\" -t ios \\"
echo "    -u <Apple ID email> -p <app-specific-password>"
echo ""
echo "Билд появится в App Store Connect → TestFlight через несколько минут"
echo "(внутренним тестировщикам — сразу, без Beta App Review)."
