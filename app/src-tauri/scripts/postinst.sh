#!/bin/bash
# Выполняется apt/dpkg сразу после установки .deb, уже от root — поэтому
# polkit-правило появляется ДО первого запуска приложения, и pkexec не
# спрашивает пароль вообще ни разу (раньше это происходило через
# install-polkit-rule в privileged_helper.sh при первом подключении —
# тот путь остаётся как fallback для апгрейда без переустановки).
set -e

mkdir -p /etc/polkit-1/rules.d
cat > /etc/polkit-1/rules.d/49-vrox-vpn-tauri.rules << 'POLKIT'
polkit.addRule(function(action, subject) {
    if (action.id != "org.freedesktop.policykit.exec") {
        return;
    }
    if (!subject.isInGroup("sudo")) {
        return;
    }
    var allowed = [
        "/usr/lib/vrox.vpn/resources/privileged_helper.sh",
        "/usr/bin/vroxcore"
    ];
    if (allowed.indexOf(action.lookup("program")) !== -1) {
        return polkit.Result.YES;
    }
});
POLKIT

# UDP-буферы сокетов для QUIC (hysteria2): дефолт Ubuntu 208 КБ, quic-go
# просит ~7 МБ. Без этого на 250+ Мбит/с ядро роняет пакеты и BBR сервера
# сбрасывает скорость — поймано на шлюзе vroxgw (разгон до 300 и обвал до
# 10-70). Значение — рекомендация quic-go (wiki «UDP Buffer Sizes»).
cat > /etc/sysctl.d/60-vrox-vpn.conf << 'SYSCTL'
net.core.rmem_max = 7500000
net.core.wmem_max = 7500000
SYSCTL
sysctl -q -p /etc/sysctl.d/60-vrox-vpn.conf || true

exit 0
