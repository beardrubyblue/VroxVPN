"""Общий помощник для запуска core/privileged_helper.sh через pkexec.

Все привилегированные операции приложения (sysctl, kill чужого процесса,
удаление TUN-интерфейса, nftables, перезапись /etc/resolv.conf, установка
.deb-обновления) идут через ОДИН скрипт с фиксированным набором подкоманд.
Polkit разрешает passwordless pkexec только для точного пути этого файла —
никаких системных утилит (sh/kill/ip/nft/sysctl/apt-get) напрямую."""
import os
import subprocess
from pathlib import Path

# Путь ВСЕГДА фиксирован, а не вычисляется относительно текущего main.py.
# Раньше он указывал на core/privileged_helper.sh рядом с тем main.py,
# который сейчас выполняется — то есть при запуске из dev-чекаута путь
# отличался от того, что записан в polkit-правиле (которое всегда ссылается
# на установленную копию). При несовпадении пути pkexec не находит
# подходящего passwordless-правила и откатывается на интерактивный запрос
# пароля — именно это происходило при старте (loosen-rp-filter) и при
# отключении (kill-hysteria/delete-tun). postinst .deb-пакета всегда кладёт
# helper именно по этому пути — независимо от того, откуда запускается сам
# main.py — поэтому правило отныне ровно одно и совпадает всегда.
HELPER_SCRIPT = Path("/opt/vroxory-vpn/core/privileged_helper.sh")


def run_privileged(
    args: list,
    input_data: str = None,
    timeout: float = 10.0,
) -> subprocess.CompletedProcess:
    """Запускает privileged_helper.sh с заданными аргументами.

    Если текущий процесс уже root — выполняет напрямую, иначе через pkexec.
    По умолчанию ограничено таймаутом: pkexec/polkitd иногда отвечают не
    мгновенно даже при passwordless-авторизации (несколько секунд), а без
    таймаута зависший вызов блокировал бы поток навсегда — например,
    цепочку disconnect() -> выход из приложения при клике "Выход" в трее.
    При таймауте возвращается синтетический неуспешный результат вместо
    выброса исключения — вызывающему коду не нужно оборачивать каждый
    вызов в try/except.
    """
    if not HELPER_SCRIPT.exists():
        msg = f"{HELPER_SCRIPT} не найден — установи .deb-пакет vrox.vpn (см. README)"
        print(f"[privileged] {msg}")
        return subprocess.CompletedProcess([str(HELPER_SCRIPT)] + args, returncode=127, stdout="", stderr=msg)

    cmd = [str(HELPER_SCRIPT)] + args
    if os.geteuid() != 0:
        cmd = ["pkexec"] + cmd
    try:
        return subprocess.run(
            cmd,
            input=input_data,
            text=True,
            capture_output=True,
            timeout=timeout,
        )
    except subprocess.TimeoutExpired:
        print(f"[privileged] таймаут выполнения: {' '.join(args)}")
        return subprocess.CompletedProcess(cmd, returncode=124, stdout="", stderr="timeout")
