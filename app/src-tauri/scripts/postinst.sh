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

exit 0
