#!/bin/bash
set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"
BUILD_DIR="$SCRIPT_DIR/build"
VERSION="2.1.0"
ARCH="amd64"
PKG_NAME="vroxory-vpn_${VERSION}_${ARCH}"

echo "═══════════════════════════════════"
echo "  Сборка Vroxory VPN .deb пакета"
echo "═══════════════════════════════════"

# Чистим
rm -rf "$BUILD_DIR"
mkdir -p "$BUILD_DIR/$PKG_NAME"

# Копируем DEBIAN/
cp -r "$SCRIPT_DIR/DEBIAN" "$BUILD_DIR/$PKG_NAME/"
chmod 755 "$BUILD_DIR/$PKG_NAME/DEBIAN/postinst"
chmod 755 "$BUILD_DIR/$PKG_NAME/DEBIAN/prerm"

# Копируем файлы приложения → /opt/vroxory-vpn/
mkdir -p "$BUILD_DIR/$PKG_NAME/opt/vroxory-vpn"
cp -r "$PROJECT_DIR/"{main.py,core,ui,requirements.txt} \
    "$BUILD_DIR/$PKG_NAME/opt/vroxory-vpn/"
find "$BUILD_DIR/$PKG_NAME/opt/vroxory-vpn" -name "__pycache__" -type d -exec rm -rf {} +
chmod +x "$BUILD_DIR/$PKG_NAME/opt/vroxory-vpn/core/privileged_helper.sh"

# Исполняемый wrapper → /usr/local/bin/vroxory-vpn
mkdir -p "$BUILD_DIR/$PKG_NAME/usr/local/bin"
cat > "$BUILD_DIR/$PKG_NAME/usr/local/bin/vroxory-vpn" << 'EOF'
#!/bin/bash
exec python3 /opt/vroxory-vpn/main.py "$@"
EOF
chmod +x "$BUILD_DIR/$PKG_NAME/usr/local/bin/vroxory-vpn"

# .desktop файл → /usr/share/applications/
mkdir -p "$BUILD_DIR/$PKG_NAME/usr/share/applications"
cat > "$BUILD_DIR/$PKG_NAME/usr/share/applications/vroxory-vpn.desktop" << EOF
[Desktop Entry]
Name=Vroxory VPN
Comment=Hysteria2 VPN клиент
Exec=vroxory-vpn
Icon=network-vpn
Terminal=false
Type=Application
Categories=Network;Security;
Keywords=vpn;hysteria;tun;
StartupNotify=true
EOF

# Собираем .deb
cd "$BUILD_DIR"
dpkg-deb --root-owner-group --build "$PKG_NAME"
mv "$PKG_NAME.deb" "$PROJECT_DIR/"

echo ""
echo "✓ Готово: $PROJECT_DIR/${PKG_NAME}.deb"
echo ""
echo "Установка:"
echo "  sudo apt install ./${PKG_NAME}.deb"

# ── Публикация (только если передан флаг --publish) ──
if [[ "$1" == "--publish" ]]; then
    echo ""
    echo "▶ Публикация..."

    # 1. Проверяем gh CLI
    if ! command -v gh &> /dev/null; then
        echo "  ✗ GitHub CLI (gh) не установлен. Установи: https://cli.github.com"
        exit 1
    fi

    # 2. Создаём GitHub Release и загружаем .deb
    gh release create "v${VERSION}" \
        "$PROJECT_DIR/vroxory-vpn_${VERSION}_${ARCH}.deb" \
        --title "Vroxory VPN v${VERSION}" \
        --notes "Обновление версии ${VERSION}" \
        --repo "beardrubyblue/VroxVPN"

    DEB_URL="https://github.com/beardrubyblue/VroxVPN/releases/download/v${VERSION}/vroxory-vpn_${VERSION}_${ARCH}.deb"

    # sha256 .deb-файла — AppUpdater проверяет его перед apt-get install,
    # чтобы скачанный файл нельзя было незаметно подменить отдельно от
    # version.json (например, при компрометации только хостинга релизов)
    DEB_SHA256="$(sha256sum "$PROJECT_DIR/vroxory-vpn_${VERSION}_${ARCH}.deb" | awk '{print $1}')"

    # 3. Создаём version.json
    cat > "$PROJECT_DIR/version.json" << EOF
{
  "version": "${VERSION}",
  "download_url": "${DEB_URL}",
  "sha256": "${DEB_SHA256}",
  "changelog": "Обновление до версии ${VERSION}",
  "min_version": "1.0.0",
  "released_at": "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
}
EOF

    echo ""
    echo "✓ version.json создан: $PROJECT_DIR/version.json"
    echo ""
    echo "Следующий шаг — загрузи version.json на сервер:"
    echo "  scp $PROJECT_DIR/version.json user@net.vroxory.com:/var/www/vpn/version.json"
    echo ""
    echo "Или для GitHub fallback:"
    echo "  git add version.json && git commit -m 'release v${VERSION}' && git push"
fi
