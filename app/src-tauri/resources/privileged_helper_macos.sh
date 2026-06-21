#!/bin/bash
# macOS-вариант privileged_helper.sh — запускается от root через
# `osascript ... with administrator privileges` (см. engine/macos.rs),
# а не через pkexec/polkit, которых на macOS нет.
#
# ⚠ НЕ ПРОВЕРЕНО НА РЕАЛЬНОМ MACOS — особенно подкоманды pf-apply/
# pf-restore: семантика pf-анкоров и поведение `pfctl -f -` без ссылки
# из основного ruleset нужно вживую перепроверить на Mac (см. чеклист
# в docs/MACOS_PORT.md) перед тем, как доверять этому как killswitch.
set -euo pipefail

PF_SAVE_PATH="/private/tmp/vroxory-vpn-pf-backup.conf"

cmd="${1:-}"
shift || true

case "$cmd" in
    kill-hysteria)
        signal="${1:?missing signal}"
        config="${2:?missing config path}"
        case "$signal" in
            TERM|KILL) ;;
            *) echo "недопустимый сигнал: $signal" >&2; exit 1 ;;
        esac
        case "$config" in
            /tmp/vroxory-vpn/*.yaml) ;;
            *) echo "недопустимый путь конфига: $config" >&2; exit 1 ;;
        esac
        # без пробела после "vroxcore": реальный sidecar называется
        # vroxcore-aarch64-apple-darwin/vroxcore-x86_64-apple-darwin (dev)
        # либо просто vroxcore (бандл) — "vroxcore .*" не матчил ни то,
        # ни другое надёжно
        pkill "-$signal" -f "vroxcore.*--config $config" || true
        ;;

    is-running)
        config="${1:?missing config path}"
        case "$config" in
            /tmp/vroxory-vpn/*.yaml) ;;
            *) echo "недопустимый путь конфига: $config" >&2; exit 1 ;;
        esac
        pgrep -f "vroxcore.*--config $config" > /dev/null
        ;;

    kill-all-hysteria)
        # вызывается при старте приложения — подчищает осиротевший
        # root-процесс vroxcore от предыдущего краша
        pkill -TERM -f "vroxcore.*--config /tmp/vroxory-vpn/" || true
        ;;

    pf-apply)
        # ⚠ НЕ ПРОВЕРЕНО. Идея: вместо "разрешить только TUN" (как на
        # Linux с nftables, где интерфейс называется предсказуемо)
        # блокируем исходящий трафик на ФИЗИЧЕСКИХ интерфейсах кроме
        # как до самого VPN-сервера — потому что имя utun-интерфейса
        # на macOS назначается ядром динамически (utun0, utun1, ...),
        # и мы не можем знать его заранее так же, как на Linux.
        server_ip="${1:?missing server ip}"
        case "$server_ip" in
            ''|*[!0-9.]*) echo "недопустимый ip: $server_ip" >&2; exit 1 ;;
        esac

        physical_ifaces="$(ifconfig -l | tr ' ' '\n' | grep -vE '^(lo|utun|gif|stf|bridge|ap|awdl|llw|p2p|pdp_ip|en[0-9]+\.)' || true)"
        if [[ -z "$physical_ifaces" ]]; then
            echo "не нашёл физических интерфейсов для kill switch" >&2
            exit 1
        fi

        if [[ ! -f "$PF_SAVE_PATH" ]]; then
            pfctl -sr > "$PF_SAVE_PATH" 2>/dev/null || true
            chmod 600 "$PF_SAVE_PATH"
        fi

        {
            for ifc in $physical_ifaces; do
                echo "block return out on $ifc all"
                echo "pass out quick on $ifc to $server_ip"
                echo "pass out quick on $ifc to 192.168.0.0/16"
                echo "pass out quick on $ifc to 10.0.0.0/8"
                echo "pass out quick on $ifc to 172.16.0.0/12"
            done
        } | pfctl -f -
        pfctl -e 2>/dev/null || true
        ;;

    pf-restore)
        if [[ -f "$PF_SAVE_PATH" ]]; then
            pfctl -f "$PF_SAVE_PATH" || true
            rm -f "$PF_SAVE_PATH"
        else
            pfctl -d 2>/dev/null || true
        fi
        ;;

    *)
        echo "неизвестная команда: $cmd" >&2
        exit 1
        ;;
esac
