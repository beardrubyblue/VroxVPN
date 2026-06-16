#!/bin/bash
# Единая точка входа для всех привилегированных операций VroxVPN.
#
# Полный путь к этому файлу — единственное, что разрешено выполнять через
# pkexec без пароля (см. polkit-правило в packaging/deb/DEBIAN/postinst и
# scripts/install.sh). Раньше правило разрешало pkexec для любых программ,
# чей путь содержал подстроки "/bin/sh", "/kill", "/ip", "/nft", "/sysctl",
# "/apt-get" — это давало passwordless root для произвольных команд любому
# процессу, запущенному пользователем из группы sudo. Теперь polkit матчит
# ТОЛЬКО точный путь к этому скрипту, а сам скрипт принимает только
# конкретные подкоманды с провалидированными аргументами — никакого
# произвольного выполнения команд через него быть не может.
set -euo pipefail

TUN_IFACE="tun-vroxory"
KILLSWITCH_TABLE="vroxory_killswitch"
UPDATE_DEB_PATH="/tmp/vroxory-update.deb"

cmd="${1:-}"
shift || true

case "$cmd" in
    loosen-rp-filter)
        # без этого ядро дропает ответы из TUN из-за строгого reverse-path filter
        sysctl -w net.ipv4.conf.all.rp_filter=2 net.ipv4.conf.default.rp_filter=2
        ;;

    kill-process)
        signal="${1:?missing signal}"
        pid="${2:?missing pid}"
        case "$signal" in
            TERM|KILL) ;;
            *) echo "недопустимый сигнал: $signal" >&2; exit 1 ;;
        esac
        if ! [[ "$pid" =~ ^[0-9]+$ ]]; then
            echo "недопустимый pid: $pid" >&2
            exit 1
        fi
        kill "-$signal" "$pid"
        ;;

    delete-tun)
        iface="${1:?missing interface}"
        if [[ "$iface" != "$TUN_IFACE" ]]; then
            echo "недопустимый интерфейс: $iface" >&2
            exit 1
        fi
        ip link delete "$iface"
        ;;

    nft-apply)
        # правила читаются из stdin — формирует их наш Python-код
        nft -f -
        ;;

    nft-delete-table)
        table="${1:?missing table}"
        if [[ "$table" != "$KILLSWITCH_TABLE" ]]; then
            echo "недопустимая таблица: $table" >&2
            exit 1
        fi
        nft delete table inet "$table"
        ;;

    write-resolv-conf)
        # новое содержимое /etc/resolv.conf читается из stdin как есть —
        # никакой shell-интерполяции, поэтому экранирование не нужно
        rm -f /etc/resolv.conf
        cat > /etc/resolv.conf
        ;;

    relink-resolv-conf)
        target="${1:?missing target}"
        rm -f /etc/resolv.conf
        ln -sf "$target" /etc/resolv.conf
        ;;

    apt-install)
        path="${1:?missing path}"
        if [[ "$path" != "$UPDATE_DEB_PATH" ]]; then
            echo "недопустимый путь: $path" >&2
            exit 1
        fi
        apt-get install -y "$path"
        ;;

    *)
        echo "неизвестная команда: $cmd" >&2
        exit 1
        ;;
esac
