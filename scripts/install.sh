#!/bin/bash
set -e
sudo apt-get update -q
sudo apt-get install -y python3-gi python3-gi-cairo gir1.2-gtk-4.0 gir1.2-adw-1 \
    gir1.2-gtk-3.0 gir1.2-ayatanaappindicator3-0.1 \
    python3-pip polkitd pkexec nftables
pip3 install --user --break-system-packages requests PyYAML pillow

# polkit правило чтобы pkexec не спрашивал пароль каждый раз:
# - hysteria2 (запуск TUN-клиента)
# - sysctl (ослабление rp_filter, иначе ядро дропает ответы из TUN)
# - kill (отключение запущенного от root процесса hysteria2)
# - ip (удаление TUN-интерфейса при отключении)
# - nft (kill switch через nftables)
# - apt-get (авто-обновление самого приложения)
sudo tee /etc/polkit-1/rules.d/49-vroxory-vpn.rules > /dev/null << 'POLKIT'
polkit.addRule(function(action, subject) {
    if (action.id == "org.freedesktop.policykit.exec" && subject.isInGroup("sudo")) {
        var program = action.lookup("program");
        var allowed = ["hysteria2", "/sysctl", "/kill", "/ip", "/nft", "/apt-get"];
        for (var i = 0; i < allowed.length; i++) {
            if (program.indexOf(allowed[i]) !== -1) {
                return polkit.Result.YES;
            }
        }
    }
});
POLKIT

# .desktop файл
APP_DIR="$(cd "$(dirname "$0")/.." && pwd)"
mkdir -p "$HOME/.local/share/applications"
cat > "$HOME/.local/share/applications/vroxory-vpn.desktop" << EOF
[Desktop Entry]
Name=Vroxory VPN
Exec=python3 $APP_DIR/main.py
Icon=network-vpn
Terminal=false
Type=Application
Categories=Network;
EOF

chmod +x "$APP_DIR/main.py"
echo "✓ Установка завершена. Запуск: python3 $APP_DIR/main.py"
