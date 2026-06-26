#!/bin/bash
# Сборка для TestFlight: Go-фреймворк → Xcode (.appex, Apple
# Distribution + App Store профиль) → Tauri (.app, release) →
# встраивание .appex и пересборка подписи всего .app под Apple
# Distribution → упаковка в .pkg (Mac Installer Distribution) →
# загрузка в App Store Connect через `xcrun altool`.
#
# Предпосылки (см. docs/ARCHITECTURE.md, раздел TestFlight):
#   1. Сертификат "Apple Distribution: ..." — Xcode → Settings →
#      Accounts → Manage Certificates → "+" → Apple Distribution.
#   2. Сертификат "3rd Party Mac Developer Installer: ..." — через CSR
#      (Keychain Access → Certificate Assistant → Request a Certificate
#      From a Certificate Authority) на developer.apple.com → Certificates
#      → "+" → Mac Installer Distribution.
#   3. Два provisioning-профиля типа "Mac App Store Connect" —
#      developer.apple.com → Profiles → "+" — для com.vroxory.vpn и
#      com.vroxory.vpn.tunnel, установлены (двойной клик или вручную в
#      ~/Library/Developer/Xcode/UserData/Provisioning Profiles/).
#   4. Запись приложения в App Store Connect (My Apps → "+" → New App,
#      bundle id com.vroxory.vpn) — без этого altool не примет загрузку.
#   5. App-specific password для твоего Apple ID (appleid.apple.com →
#      Sign-In and Security → App-Specific Passwords) — altool не
#      принимает обычный пароль аккаунта.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
APP_PATH="$REPO_ROOT/app/src-tauri/target/release/bundle/macos/vrox.vpn.app"
VERSION="$(node -p "require('$REPO_ROOT/app/src-tauri/tauri.conf.json').version")"
PKG_DIR="$REPO_ROOT/app/src-tauri/target/release/bundle/pkg"
PKG_PATH="$PKG_DIR/vrox.vpn_${VERSION}.pkg"

APP_DISTRIBUTION_IDENTITY="${APP_DISTRIBUTION_IDENTITY:-Apple Distribution: Aleksandr Sysoev (QRZT5R3Q28)}"
INSTALLER_IDENTITY="${INSTALLER_IDENTITY:-3rd Party Mac Developer Installer: Aleksandr Sysoev (QRZT5R3Q28)}"
HOST_PROFILE_NAME="${HOST_PROFILE_NAME:-vrox.vpn App Store}"

echo "==> [1/5] Go-фреймворк (GoNetunnel.xcframework)"
"$SCRIPT_DIR/build-go-framework.sh" macos

echo "==> [2/5] Xcode .appex (Release — теперь Apple Distribution + App Store профиль)"
xcodebuild -project "$SCRIPT_DIR/VroxVPNNetworkExtension.xcodeproj" \
    -scheme VroxVPNHost -configuration Release -allowProvisioningUpdates build \
    | tail -5

echo "==> [3/5] Tauri .app (release)"
(cd "$REPO_ROOT/app" && pnpm tauri build)

echo "==> [4/5] Встраивание .appex + переподпись всего .app под Apple Distribution"
RELEASE_APPEX="$(find ~/Library/Developer/Xcode/DerivedData -path '*/VroxVPNNetworkExtension-*/Build/Products/Release/VroxTunnelExtension.appex' -maxdepth 8 2>/dev/null | head -1)"
if [[ -z "$RELEASE_APPEX" ]]; then
    echo "✗ VroxTunnelExtension.appex не найден — шаг [2/5] не собрался?" >&2
    exit 1
fi
# Ищем профиль по точному имени (которое мы сами задали при создании
# на developer.apple.com) — см. embed-into-tauri-app.sh для похожего
# поиска, но по bundle id, а не по имени.
HOST_PROFILE="$(for f in ~/Library/Developer/Xcode/UserData/Provisioning\ Profiles/*.provisionprofile; do
    name="$(security cms -D -i "$f" 2>/dev/null | plutil -extract Name xml1 -o - - 2>/dev/null | sed -n 's/.*<string>\(.*\)<\/string>.*/\1/p')"
    [[ "$name" == "$HOST_PROFILE_NAME" ]] && echo "$f" && break
done)"
if [[ -z "$HOST_PROFILE" ]]; then
    echo "✗ Provisioning-профиль \"$HOST_PROFILE_NAME\" не найден среди установленных" >&2
    exit 1
fi

rm -rf "$APP_PATH/Contents/PlugIns/VroxTunnelExtension.appex"
mkdir -p "$APP_PATH/Contents/PlugIns"
ditto "$RELEASE_APPEX" "$APP_PATH/Contents/PlugIns/VroxTunnelExtension.appex"
cp "$HOST_PROFILE" "$APP_PATH/Contents/embedded.provisionprofile"

# .appex НЕ трогаем (уже подписан Distribution-сертификатом и своим
# App Store профилем на шаге [2/5], тем же кодом, что и
# embed-into-tauri-app.sh для прямой раздачи). Подписываем только
# внешний .app — с App Sandbox entitlements (см. macos/entitlements.plist).
codesign --force --options runtime --sign "$APP_DISTRIBUTION_IDENTITY" \
    --entitlements "$REPO_ROOT/app/src-tauri/macos/entitlements.plist" \
    "$APP_PATH"
codesign --verify --deep --strict --verbose=2 "$APP_PATH"

echo "==> [5/5] Упаковка в .pkg (Mac Installer Distribution)"
mkdir -p "$PKG_DIR"
rm -f "$PKG_PATH"
productbuild --component "$APP_PATH" /Applications --sign "$INSTALLER_IDENTITY" "$PKG_PATH"

echo ""
echo "✓ Готово: $PKG_PATH"
echo ""
echo "Для загрузки в App Store Connect (нужен app-specific password —"
echo "appleid.apple.com → Sign-In and Security → App-Specific Passwords):"
echo "  xcrun altool --upload-app -f \"$PKG_PATH\" -t macos \\"
echo "    -u <твой Apple ID email> -p <app-specific-password>"
echo ""
echo "После загрузки билд появится в App Store Connect → TestFlight"
echo "обычно через несколько минут (может потребоваться обработка/Beta"
echo "App Review для внешних тестеров, для внутренних — сразу доступен)."
