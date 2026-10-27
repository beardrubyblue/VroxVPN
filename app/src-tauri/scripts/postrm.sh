#!/bin/bash
# Убираем polkit-правило при удалении пакета — иначе остаётся
# осиротевший файл, разрешающий passwordless pkexec для путей, которых
# уже нет на диске.
set -e

rm -f /etc/polkit-1/rules.d/49-vrox-vpn-tauri.rules
rm -f /etc/sysctl.d/60-vrox-vpn.conf

exit 0
