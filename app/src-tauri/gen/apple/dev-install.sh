#!/bin/bash
# Быстрый цикл правка→тест на физическом iPhone по кабелю, без
# TestFlight. `tauri ios dev` сам по себе не работает (тот же сломанный
# auto-export, что и у `tauri ios build --export-method`, см. git log
# этого файла) — собираем debug-архив сами и экспортируем с Development-
# подписью вручную, как и Release-путь в ../../../macos-ext/build-
# testflight-ios.sh.
#
# Предпосылки: профили "Dev vrox.vpn Apple Store" / "Dev vrox.vpn tunnel
# Apple Store" (тип iOS App Development, capability Network Extensions)
# установлены локально, устройство подключено и доверяет этому Mac.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
APP_DIR="$(cd "$SCRIPT_DIR/../../.." && pwd)"
DEVICE_ID="${1:?Использование: ./dev-install.sh <device-id> (см. xcrun devicectl list devices)}"

rm -rf "$SCRIPT_DIR/build/app_iOS.xcarchive" "$SCRIPT_DIR/build/export-dev"

echo "==> [1/3] Сборка debug-архива"
(cd "$APP_DIR" && APPLE_DEVELOPMENT_TEAM=QRZT5R3Q28 ./node_modules/.bin/tauri ios build --debug 2>&1 | tail -20) || true

echo "==> [2/3] Экспорт с Development-подписью"
xcodebuild -exportArchive \
    -archivePath "$SCRIPT_DIR/build/app_iOS.xcarchive" \
    -exportPath "$SCRIPT_DIR/build/export-dev" \
    -exportOptionsPlist "$SCRIPT_DIR/ExportOptionsDev.plist" \
    -allowProvisioningUpdates

echo "==> [3/3] Установка + запуск на устройстве"
xcrun devicectl device install app --device "$DEVICE_ID" "$SCRIPT_DIR/build/export-dev/vrox.vpn.ipa"
xcrun devicectl device process launch --device "$DEVICE_ID" com.vroxory.vpn

echo "✓ Готово"
