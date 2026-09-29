#!/bin/bash
# Собирает нашу версию hysteria2 (форк apernet/hysteria с патчем
# directDomains — см. direct-domains.patch) и публикует её как GitHub
# Release asset в этом же репозитории, отдельным тегом от релизов
# самого приложения.
set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
UPSTREAM_TAG="app/v2.12.3"
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
# macOS/NetworkExtension).
#
# ⚠ ВАЖНО: путь именно app/netunnel, НЕ app/internal/netunnel. Проверено
# на реальном Mac: `gomobile bind` генерирует свою obj-c/swift wrapper-
# package во ВРЕМЕННОМ отдельном Go-модуле — а Go запрещает импорт
# internal-пакетов кодом снаружи дерева модуля, которому принадлежит
# internal/ (см. https://go.dev/doc/go1.4#internalpackages). Раньше
# netunnel лежал в app/internal/netunnel — `gomobile bind` падал с "use of
# internal package ... not allowed". Сам netunnel при этом всё ещё может
# импортировать app/internal/utils (ConvBandwidth) — он остаётся в дереве
# модуля github.com/apernet/hysteria/app/v2, просто не помечен internal
# сам по себе.
mkdir -p app/netunnel
cp "$SCRIPT_DIR/netunnel/"*.go app/netunnel/

# go.work корня апстрима фиксирует `go 1.26.0` (на v2.12.3; на v2.9.2 было
# 1.24.0) — gvisor.dev/gvisor требует >= 1.26.3 (проверено вживую). go.work — не наш файл (часть
# upstream-репозитория hysteria, regenerируется при каждом git clone
# заново), поэтому правим его тут же, а не один раз руками.
go work edit -go=1.26.4 "$BUILD_DIR/hysteria/go.work"

cd app
# miekg/dns (нужен directmatch.go) раньше пинился здесь на v1.1.59 — на
# v2.9.2 апстрим его не тянул. С v2.11 апстрим сам зависит от v1.1.72
# (через обновлённый ACME-стек), и старый пин при переходе на v2.12.3
# ОТКАТЫВАЛ certmagic 0.25→0.21 и acmez — убран, берём версию апстрима.
# gvisor.dev/gvisor@latest — НЕНАДЁЖНО: на момент проверки резолвился в
# снэпшот с реальным конфликтом package-имён в pkg/tcpip/stack
# (bridge_test.go объявлен как `package bridge_test`, не `stack_test` —
# подобно тому, как это организовано у них во внутреннем Bazel-сборщике,
# но ломает обычный `go build`/`go vet`). Версия ниже — конкретный
# коммит, который реально использует в проде tailscale.com (см. их
# go.mod на pkg.go.dev) — не угадывание, а заведомо собирающийся пин.
go get gvisor.dev/gvisor@v0.0.0-20260224225140-573d5e7127a8
go mod tidy

echo "→ проверяю netunnel (go vet, кросс-проверка типов под NE-путь)..."
go vet ./netunnel/...

# --skip-cli — используется ios/build-go-framework.sh: ему нужно
# только подготовленное выше исходное дерево (клон+патч+netunnel/+
# go.sum) для `gomobile bind`, сами CLI-бинарники этого скрипта он не
# запускает. Раньше здесь же собирались ещё и darwin-amd64/darwin-
# arm64 — удалены: macOS/iOS перешли на NetworkExtension (sidecar+
# osascript+pf удалены целиком из engine/macos/mod.rs), эти бинарники
# не используются ничем в продукте, только занимали время сборки.
# linux-amd64/linux-arm64 остаются — это и есть `vroxcore`,
# externalBin-sidecar для Linux-сборки (tauri.conf.json).
if [[ "$1" == "--skip-cli" ]]; then
    cd "$SCRIPT_DIR"
    echo "✓ Исходное дерево подготовлено (--skip-cli, без сборки бинарников)"
    exit 0
fi

rm -f "$BUILD_DIR/hashes.txt"
ASSETS=()
for target in linux:amd64 linux:arm64; do
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
