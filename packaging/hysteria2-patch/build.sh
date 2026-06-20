#!/bin/bash
# Собирает нашу версию hysteria2 (форк apernet/hysteria с патчем
# directDomains — см. direct-domains.patch) и публикует её как GitHub
# Release asset в этом же репозитории, отдельным тегом от релизов
# самого приложения.
set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
UPSTREAM_TAG="app/v2.9.2"
PATCH_REVISION="1"
VERSION="${UPSTREAM_TAG#app/}-vroxory${PATCH_REVISION}"
TAG="hysteria2-fork-${UPSTREAM_TAG#app/}-${PATCH_REVISION}"
BUILD_DIR="$SCRIPT_DIR/build"

echo "═══════════════════════════════════"
echo "  Сборка vrox.vpn hysteria2-fork"
echo "  upstream: ${UPSTREAM_TAG}, патч ревизия: ${PATCH_REVISION}"
echo "═══════════════════════════════════"

rm -rf "$BUILD_DIR"
mkdir -p "$BUILD_DIR"
git clone --depth 1 https://github.com/apernet/hysteria.git "$BUILD_DIR/hysteria"
cd "$BUILD_DIR/hysteria"
git fetch --depth 1 origin tag "$UPSTREAM_TAG"
git checkout "$UPSTREAM_TAG"

git apply --include="app/cmd/client.go" "$SCRIPT_DIR/direct-domains.patch"
git apply --include="app/internal/tun/server.go" "$SCRIPT_DIR/direct-domains.patch"
cp "$SCRIPT_DIR/directmatch.go" "$SCRIPT_DIR/dnssniff.go" app/internal/tun/

cd app
go get github.com/miekg/dns@v1.1.59
go mod tidy

rm -f "$BUILD_DIR/hashes.txt"
ASSETS=()
for arch in amd64 arm64; do
    asset_name="hysteria2-vroxory-linux-${arch}"
    echo "→ собираю ${asset_name}..."
    GOOS=linux GOARCH="$arch" go build \
        -ldflags "-X github.com/apernet/hysteria/app/v2/cmd.appVersion=${VERSION}" \
        -o "$BUILD_DIR/$asset_name" .
    sha="$(sha256sum "$BUILD_DIR/$asset_name" | awk '{print $1}')"
    echo "$sha  $asset_name" >> "$BUILD_DIR/hashes.txt"
    ASSETS+=("$BUILD_DIR/$asset_name")
done
cd "$SCRIPT_DIR"

echo ""
echo "✓ Собрано:"
cat "$BUILD_DIR/hashes.txt"

if [[ "$1" == "--publish" ]]; then
    echo ""
    echo "▶ Публикация в beardrubyblue/VroxVPN, тег ${TAG}..."
    gh release create "$TAG" \
        "${ASSETS[@]}" \
        "$BUILD_DIR/hashes.txt" \
        --title "hysteria2-fork ${UPSTREAM_TAG} (vroxory patch ${PATCH_REVISION})" \
        --notes "Наша сборка hysteria2 (форк ${UPSTREAM_TAG}) с патчем directDomains — обход VPN по списку доменов через DNS-сниффинг на реальном интерфейсе, без изменений в системной таблице маршрутизации. См. packaging/hysteria2-patch/direct-domains.patch." \
        --repo "beardrubyblue/VroxVPN"
    echo ""
    echo "✓ Опубликовано: https://github.com/beardrubyblue/VroxVPN/releases/tag/${TAG}"
fi
