#!/bin/bash
# Сборка для TestFlight (iOS/iPadOS): Go-фреймворк → archive (.xcarchive,
# Apple Distribution + "iOS App Store" профили) → export (.ipa) →
# загрузка в App Store Connect через `xcrun altool`.
#
# Проще macOS-варианта (build-testflight.sh) — здесь нет отдельного
# Tauri-слоя: VroxVPNHost-iOS встраивает VroxTunnelExtension-iOS сам
# (xcodegen `dependencies: embed: true`), archive/export уже отдают
# готовый подписанный .ipa, не нужно вручную codesign/productbuild.
#
# Предпосылки (см. docs/ARCHITECTURE.md, раздел TestFlight):
#   1. Capability Network Extensions включена для iOS-платформы на App
#      ID com.vroxory.vpn и com.vroxory.vpn.tunnel (developer.apple.com
#      → Identifiers).
#   2. Два provisioning-профиля типа "App Store Connect" (iOS) —
#      developer.apple.com → Profiles → "+" — имена "vrox.vpn iOS App
#      Store" и "vrox.vpn tunnel iOS App Store", установлены.
#   3. Сертификат "Apple Distribution: ..." — тот же, что и для macOS
#      (см. build-testflight.sh), он общий для iOS+macOS.
#   4. iOS-платформа добавлена в запись приложения в App Store Connect
#      (My Apps → vrox.vpn → "+" платформа, либо отдельная запись с тем
#      же bundle id com.vroxory.vpn) — без этого altool не примет
#      загрузку.
#   5. App-specific password для Apple ID (см. build-testflight.sh).
#   6. Xcode → Settings → Components → iOS Platform скачана.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
ARCHIVE_PATH="$SCRIPT_DIR/build-ios/VroxVPNHost-iOS.xcarchive"
EXPORT_PATH="$SCRIPT_DIR/build-ios/export"
EXPORT_OPTIONS_PLIST="$SCRIPT_DIR/build-ios/ExportOptions.plist"

APP_PROFILE_NAME="${APP_PROFILE_NAME:-vrox.vpn iOS App Store}"
TUNNEL_PROFILE_NAME="${TUNNEL_PROFILE_NAME:-vrox.vpn tunnel iOS App Store}"
TEAM_ID="${TEAM_ID:-QRZT5R3Q28}"

# Тот же приём, что и в build-testflight.sh — монотонный build-номер без
# отдельного счётчика на ведение руками, растёт сам с каждым коммитом.
BUILD_NUMBER="$(git -C "$REPO_ROOT" rev-list --count HEAD)"
echo "→ build-номер (CFBundleVersion): $BUILD_NUMBER"

echo "==> [1/4] Go-фреймворк (GoNetunnel.xcframework, слайс ios)"
"$SCRIPT_DIR/build-go-framework.sh" ios,macos

mkdir -p "$SCRIPT_DIR/build-ios"
rm -rf "$ARCHIVE_PATH" "$EXPORT_PATH"

echo "==> [2/4] Archive (Release — Apple Distribution + iOS App Store профили)"
xcodebuild -project "$SCRIPT_DIR/VroxVPNNetworkExtension.xcodeproj" \
    -scheme VroxVPNHost-iOS -configuration Release -allowProvisioningUpdates \
    -archivePath "$ARCHIVE_PATH" \
    -destination "generic/platform=iOS" \
    CURRENT_PROJECT_VERSION="$BUILD_NUMBER" \
    archive | tail -10

cat > "$EXPORT_OPTIONS_PLIST" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>method</key>
    <string>app-store-connect</string>
    <key>teamID</key>
    <string>$TEAM_ID</string>
    <key>signingStyle</key>
    <string>manual</string>
    <key>provisioningProfiles</key>
    <dict>
        <key>com.vroxory.vpn</key>
        <string>$APP_PROFILE_NAME</string>
        <key>com.vroxory.vpn.tunnel</key>
        <string>$TUNNEL_PROFILE_NAME</string>
    </dict>
</dict>
</plist>
PLIST

echo "==> [3/4] Export (.ipa)"
xcodebuild -exportArchive \
    -archivePath "$ARCHIVE_PATH" \
    -exportPath "$EXPORT_PATH" \
    -exportOptionsPlist "$EXPORT_OPTIONS_PLIST" \
    -allowProvisioningUpdates | tail -10

IPA_PATH="$(find "$EXPORT_PATH" -maxdepth 1 -name '*.ipa' | head -1)"
if [[ -z "$IPA_PATH" ]]; then
    echo "✗ .ipa не найден в $EXPORT_PATH — шаг export не прошёл?" >&2
    exit 1
fi

echo ""
echo "✓ Готово: $IPA_PATH"
echo ""
echo "==> [4/4] Загрузка в App Store Connect (нужен app-specific password —"
echo "appleid.apple.com → Sign-In and Security → App-Specific Passwords):"
echo "  xcrun altool --upload-app -f \"$IPA_PATH\" -t ios \\"
echo "    -u <твой Apple ID email> -p <app-specific-password>"
echo ""
echo "После загрузки билд появится в App Store Connect → TestFlight"
echo "обычно через несколько минут (для внутренних тестеров — сразу"
echo "доступен, без Beta App Review)."
