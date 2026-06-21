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
cp "$SCRIPT_DIR/directmatch.go" "$SCRIPT_DIR/directmatch_linux.go" \
   "$SCRIPT_DIR/directmatch_darwin.go" "$SCRIPT_DIR/dnssniff_linux.go" \
   "$SCRIPT_DIR/dnssniff_darwin.go" app/internal/tun/

# netunnel — байт-слайс адаптация app/internal/tun для NetworkExtension
# (macOS/iOS): gVisor-стек без настоящего TUN-fd, биндится через
# `gomobile bind` в .xcframework для Swift. НЕ собирается в сам бинарник
# vroxcore (цикл сборки ниже её не трогает) — отдельный артефакт,
# собирается отдельно (см. docs/ARCHITECTURE.md, раздел
# macOS/NetworkExtension). ⚠ НЕ ПРОВЕРЕНО через реальный `gomobile bind`
# (нет Xcode на машине, где это писалось) — только `go build`/`go vet`.
mkdir -p app/internal/netunnel
cp "$SCRIPT_DIR/netunnel/"*.go app/internal/netunnel/

cd app
go get github.com/miekg/dns@v1.1.59
go mod tidy

echo "→ проверяю netunnel (go vet, кросс-проверка типов под NE-путь)..."
go vet ./internal/netunnel/...

rm -f "$BUILD_DIR/hashes.txt"
ASSETS=()
# os:arch — добавлен darwin для Tauri-сборки под macOS (sidecar
# vroxcore-{arch}-apple-darwin). Проверено на самом Mac: TUN-код форка
# (directmatch.go/dnssniff.go) использовал Linux-специфичные syscall
# (AF_PACKET, SO_BINDTODEVICE, /proc/net/route) и не собирался под darwin —
# вынесено в directmatch_linux.go/directmatch_darwin.go и
# dnssniff_linux.go/dnssniff_darwin.go через `//go:build`. На macOS фича
# directDomains пока отключена (defaultInterfaceName возвращает ошибку),
# остальное собирается и работает одинаково на обеих платформах.
for target in linux:amd64 linux:arm64 darwin:amd64 darwin:arm64; do
    os="${target%%:*}"
    arch="${target##*:}"
    asset_name="hysteria2-vroxory-${os}-${arch}"
    echo "→ собираю ${asset_name}..."
    GOOS="$os" GOARCH="$arch" go build \
        -ldflags "-s -w -X github.com/apernet/hysteria/app/v2/cmd.appVersion=${VERSION}" \
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
