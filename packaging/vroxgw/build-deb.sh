#!/bin/bash
# Собирает vroxgw_<версия>_amd64.deb — шлюз для Ubuntu/Debian x86_64.
#
# vroxgw собирается в Docker (rust:alpine, цель musl): бинарник статический
# и не зависит от версии glibc на VM — сборка на свежем glibc и запуск на
# старой Ubuntu иначе падали бы с «GLIBC_2.xx not found».
# vroxcore (наш форк hysteria2) — тот же, что у Linux-приложения:
# packaging/hysteria2-patch/build.sh кладёт его в build/, либо путь
# передаётся явно через VROXCORE=...
set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO="$(cd "$SCRIPT_DIR/../.." && pwd)"
VERSION="$(grep -m1 '^version' "$REPO/crates/vroxgw/Cargo.toml" | cut -d'"' -f2)"
VROXCORE="${VROXCORE:-$REPO/packaging/hysteria2-patch/build/hysteria2-vroxory-linux-amd64}"
OUT="$SCRIPT_DIR/build"
PKG="$OUT/vroxgw_${VERSION}_amd64"

if [[ ! -f "$VROXCORE" ]]; then
    echo "✗ нет $VROXCORE — соберите packaging/hysteria2-patch/build.sh или задайте VROXCORE=" >&2
    exit 1
fi

echo "→ собираю vroxgw ${VERSION} (linux/amd64, musl)..."
docker run --rm --platform linux/amd64 \
    -v "$REPO:/src" -w /src/crates/vroxgw \
    -e CARGO_TARGET_DIR=/src/crates/vroxgw/target/docker \
    rust:1-alpine sh -c "apk add --no-cache musl-dev cmake make perl >/dev/null && cargo build --release"
BIN="$REPO/crates/vroxgw/target/docker/release/vroxgw"

rm -rf "$PKG"
mkdir -p "$PKG/DEBIAN" "$PKG/usr/bin" "$PKG/usr/lib/vroxgw" "$PKG/usr/share/vroxgw" \
    "$PKG/etc/vroxgw" "$PKG/etc/sysctl.d" "$PKG/lib/systemd/system"
install -m 755 "$BIN" "$PKG/usr/bin/vroxgw"
install -m 755 "$VROXCORE" "$PKG/usr/lib/vroxgw/vroxcore"
cp -R "$REPO/app/src-tauri/resources/geoip" "$REPO/app/src-tauri/resources/geosite" "$PKG/usr/share/vroxgw/"
install -m 600 "$SCRIPT_DIR/debian/config.toml" "$PKG/etc/vroxgw/config.toml"
install -m 644 "$SCRIPT_DIR/debian/60-vroxgw.conf" "$PKG/etc/sysctl.d/60-vroxgw.conf"
install -m 644 "$SCRIPT_DIR/debian/vroxgw.service" "$PKG/lib/systemd/system/vroxgw.service"
install -m 755 "$SCRIPT_DIR/debian/postinst" "$SCRIPT_DIR/debian/prerm" "$PKG/DEBIAN/"
echo "/etc/vroxgw/config.toml" > "$PKG/DEBIAN/conffiles"
cat > "$PKG/DEBIAN/control" <<CONTROL
Package: vroxgw
Version: ${VERSION}
Architecture: amd64
Maintainer: Vroxory
Depends: nftables, iputils-ping, ca-certificates
Section: net
Priority: optional
Description: vrox.vpn gateway — шлюз hysteria2 для локальной сети
 Поднимает тоннель по подписке vrox.vpn и пропускает через него
 трафик клиентов LAN, с RU-обходом и kill switch.
CONTROL

# dpkg-deb есть только на Linux — собираем пакет в том же Docker.
docker run --rm -v "$OUT:/out" debian:stable-slim \
    dpkg-deb --root-owner-group --build "/out/$(basename "$PKG")" >/dev/null
echo "✓ $PKG.deb"
