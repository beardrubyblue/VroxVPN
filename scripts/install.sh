#!/bin/bash
set -e
sudo apt-get update -q
sudo apt-get install -y python3-gi python3-gi-cairo gir1.2-gtk-4.0 gir1.2-adw-1 \
    gir1.2-gtk-3.0 gir1.2-ayatanaappindicator3-0.1 \
    python3-pip polkitd pkexec nftables
pip3 install --user --break-system-packages requests PyYAML pillow

APP_DIR="$(cd "$(dirname "$0")/.." && pwd)"
chmod +x "$APP_DIR/core/privileged_helper.sh"

# polkit правило: passwordless pkexec ТОЛЬКО для точного пути нашего
# единого helper-скрипта и hysteria2-бинарника — без substring-matching
# по системным утилитам (sh/kill/ip/nft/sysctl/apt-get), который раньше
# давал любому процессу из группы sudo пройти pkexec без пароля вообще
# для чего угодно с этими подстроками в пути.
sudo tee /etc/polkit-1/rules.d/49-vroxory-vpn.rules > /dev/null << POLKIT
polkit.addRule(function(action, subject) {
    if (action.id != "org.freedesktop.policykit.exec") {
        return;
    }
    if (!subject.isInGroup("sudo")) {
        return;
    }
    var program = action.lookup("program");
    var allowed = [
        "$APP_DIR/core/privileged_helper.sh",
        "/usr/local/bin/hysteria2",
        "$HOME/.local/bin/hysteria2"
    ];
    if (allowed.indexOf(program) !== -1) {
        return polkit.Result.YES;
    }
});
POLKIT

# Иконки → ~/.local/share/icons/hicolor/<size>/apps/
for size in 16 32 48 64 128 256 512; do
    icon_dir="$HOME/.local/share/icons/hicolor/${size}x${size}/apps"
    mkdir -p "$icon_dir"
    cp "$APP_DIR/assets/icons/com.vroxory.vpn-${size}.png" "$icon_dir/com.vroxory.vpn.png"
done

# .desktop файл — имя файла совпадает с application_id ("com.vroxory.vpn"),
# иначе GNOME Shell не сопоставит окно с записью и в доке/Alt-Tab будет
# виден голый application_id вместо "vrox.vpn"
mkdir -p "$HOME/.local/share/applications"
cat > "$HOME/.local/share/applications/com.vroxory.vpn.desktop" << EOF
[Desktop Entry]
Name=vrox.vpn
Comment=Hysteria2 VPN клиент
Exec=python3 $APP_DIR/main.py
Icon=com.vroxory.vpn
Terminal=false
Type=Application
Categories=Network;Security;
Keywords=vpn;hysteria;tun;
StartupWMClass=com.vroxory.vpn
EOF
rm -f "$HOME/.local/share/applications/vroxory-vpn.desktop"

gtk-update-icon-cache -f "$HOME/.local/share/icons/hicolor" 2>/dev/null || true
update-desktop-database "$HOME/.local/share/applications" 2>/dev/null || true

chmod +x "$APP_DIR/main.py"
echo "✓ Установка завершена. Запуск: python3 $APP_DIR/main.py"
