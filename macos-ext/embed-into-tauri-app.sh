#!/bin/bash
# Встраивает VroxTunnelExtension.appex в уже собранный Tauri .app и
# пересобирает сквозную подпись — Фаза 6 плана перехода на
# NetworkExtension (см. docs/ARCHITECTURE.md).
#
# Tauri сам не умеет это делать (signingIdentity: null = ad-hoc подпись
# без entitlements вообще, см. находку в этой сессии) — поэтому
# отдельный шаг руками после `pnpm tauri build`, не часть его pipeline.
#
# Предпосылки:
#   1. macos-ext/Frameworks/GoNetunnel.xcframework собран
#      (./build-go-framework.sh)
#   2. macos-ext/VroxVPNNetworkExtension.xcodeproj собран хотя бы раз
#      (xcodebuild ... -scheme VroxVPNHost ...) — отсюда берём уже
#      ПРАВИЛЬНО подписанный .appex (свой собственный
#      embedded.provisionprofile + entitlements, трогать не нужно).
#   3. Провижининг-профиль для com.vroxory.vpn (главного приложения) уже
#      получен Xcode'ом при сборке VroxVPNHost (тот же bundle id) —
#      лежит в ~/Library/Developer/Xcode/UserData/Provisioning Profiles/.
#      Если профиля нет — см. PROFILE_SEARCH ниже, ничего не сделает
#      само, скрипт упадёт с понятной ошибкой.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
APP_PATH="${1:?usage: embed-into-tauri-app.sh /path/to/vrox.vpn.app}"
IDENTITY="${SIGNING_IDENTITY:-Apple Development: Aleksandr Sysoev (9HJL3Q2UJ6)}"
MAIN_BUNDLE_ID="com.vroxory.vpn"
ENTITLEMENTS="$SCRIPT_DIR/../app/src-tauri/macos/entitlements.plist"

DERIVED_DATA_APPEX="$(find ~/Library/Developer/Xcode/DerivedData -path '*/VroxVPNNetworkExtension-*/Build/Products/Debug/VroxVPNHost.app/Contents/PlugIns/VroxTunnelExtension.appex' -maxdepth 8 2>/dev/null | head -1)"
if [[ -z "$DERIVED_DATA_APPEX" ]]; then
    echo "✗ VroxTunnelExtension.appex не найден в DerivedData — сначала собери macos-ext (xcodebuild -scheme VroxVPNHost)" >&2
    exit 1
fi

PROFILE_SEARCH=~/Library/Developer/Xcode/UserData/Provisioning\ Profiles
MAIN_PROFILE=""
for f in "$PROFILE_SEARCH"/*.provisionprofile; do
    [[ -f "$f" ]] || continue
    if security cms -D -i "$f" 2>/dev/null | grep -q ">[A-Z0-9]*\.${MAIN_BUNDLE_ID}<"; then
        MAIN_PROFILE="$f"
        break
    fi
done
if [[ -z "$MAIN_PROFILE" ]]; then
    echo "✗ Провижининг-профиль для $MAIN_BUNDLE_ID не найден — собери macos-ext/VroxVPNHost хотя бы раз (тот же bundle id), Xcode получит профиль автоматически" >&2
    exit 1
fi

echo "→ .appex: $DERIVED_DATA_APPEX"
echo "→ профиль главного приложения: $MAIN_PROFILE"

mkdir -p "$APP_PATH/Contents/PlugIns"
rm -rf "$APP_PATH/Contents/PlugIns/VroxTunnelExtension.appex"
cp -R "$DERIVED_DATA_APPEX" "$APP_PATH/Contents/PlugIns/"
cp "$MAIN_PROFILE" "$APP_PATH/Contents/embedded.provisionprofile"

# .appex НЕ трогаем --deep (он уже подписан правильно, со своими
# entitlements/профилем) — подписываем только внешний .app, отдельным
# вызовом без --deep, чтобы не переподписать вложенное расширение
# entitlements главного приложения (это было бы неверно: у расширения
# свой app-sandbox + network.client/server, которых у хоста нет).
codesign --force --options runtime --sign "$IDENTITY" \
    --entitlements "$ENTITLEMENTS" \
    "$APP_PATH"

echo ""
echo "✓ Встроено и подписано: $APP_PATH"
echo ""
codesign --verify --deep --strict --verbose=2 "$APP_PATH" 2>&1
