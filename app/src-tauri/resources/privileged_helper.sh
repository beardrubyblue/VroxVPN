#!/bin/bash
# Единая точка входа для всех привилегированных операций vrox.vpn.
#
# Полный путь к этому файлу — единственное, что разрешено выполнять через
# pkexec без пароля (см. polkit-правило в packaging/deb/DEBIAN/postinst).
# Раньше правило разрешало pkexec для любых программ,
# чей путь содержал подстроки "/bin/sh", "/kill", "/ip", "/nft", "/sysctl",
# "/apt-get" — это давало passwordless root для произвольных команд любому
# процессу, запущенному пользователем из группы sudo. Теперь polkit матчит
# ТОЛЬКО точный путь к этому скрипту, а сам скрипт принимает только
# конкретные подкоманды с провалидированными аргументами — никакого
# произвольного выполнения команд через него быть не может.
set -euo pipefail

TUN_IFACE="tun-vroxory"
KILLSWITCH_TABLE="vroxory_killswitch"

cmd="${1:-}"
shift || true

case "$cmd" in
    loosen-rp-filter)
        # без этого ядро дропает ответы из TUN из-за строгого reverse-path filter
        sysctl -w net.ipv4.conf.all.rp_filter=2 net.ipv4.conf.default.rp_filter=2
        ;;

    kill-hysteria)
        # pkexec — не сам процесс hysteria2, а отдельный supervisor: сигнал,
        # отправленный pid-у, который видит Python (Popen.pid), убивает
        # только supervisor, а настоящий root-процесс hysteria2 остаётся
        # висеть и держит TUN-устройство захваченным. Поэтому ищем
        # настоящий процесс по уникальному пути конфига, а не по pid.
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
        # бинарник нашего форка называется vroxcore (sidecar Tauri-приложения),
        # а не hysteria2, как в питоновской версии — паттерн под её имя
        # здесь никогда бы не совпал
        pkill "-$signal" -f "vroxcore .*--config $config" || true
        ;;

    kill-all-hysteria)
        # вызывается при старте приложения — подчищает осиротевший
        # root-процесс vroxcore, если прошлый запуск приложения убили/
        # он крашнулся до disconnect (pkexec не наш child, поэтому сам
        # процесс этого не замечает и продолжает держать TUN)
        pkill -TERM -f "vroxcore .*--config /tmp/vroxory-vpn/" || true
        ;;

    is-running)
        # опрос для kill_client: вместо фиксированной паузы ждём
        # фактического завершения процесса перед delete-tun
        config="${1:?missing config path}"
        case "$config" in
            /tmp/vroxory-vpn/*.yaml) ;;
            *) echo "недопустимый путь конфига: $config" >&2; exit 1 ;;
        esac
        pgrep -f "vroxcore .*--config $config" > /dev/null
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

    install-polkit-rule)
        # Разрешает passwordless pkexec для ЭТОГО helper-скрипта и для
        # vroxcore — без этого Rust-приложение спрашивало бы пароль на
        # каждый отдельный pkexec-вызов (3 раза на connect, 2 на disconnect).
        # Пути фиксированные константы, а не аргумент — иначе вызывающий
        # (ещё не root) мог бы попросить разрешить passwordless root для
        # произвольного пути.
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
        ;;

    *)
        echo "неизвестная команда: $cmd" >&2
        exit 1
        ;;
esac
