"""Общий помощник для запуска core/privileged_helper.sh через pkexec.

Все привилегированные операции приложения (sysctl, kill чужого процесса,
удаление TUN-интерфейса, nftables, перезапись /etc/resolv.conf, установка
.deb-обновления) идут через ОДИН скрипт с фиксированным набором подкоманд.
Polkit разрешает passwordless pkexec только для точного пути этого файла —
никаких системных утилит (sh/kill/ip/nft/sysctl/apt-get) напрямую."""
import os
import subprocess
from pathlib import Path

HELPER_SCRIPT = Path(__file__).resolve().parent / "privileged_helper.sh"


def run_privileged(
    args: list,
    input_data: str = None,
    timeout: float = None,
) -> subprocess.CompletedProcess:
    """Запускает privileged_helper.sh с заданными аргументами.

    Если текущий процесс уже root — выполняет напрямую, иначе через pkexec.
    """
    cmd = [str(HELPER_SCRIPT)] + args
    if os.geteuid() != 0:
        cmd = ["pkexec"] + cmd
    return subprocess.run(
        cmd,
        input=input_data,
        text=True,
        capture_output=True,
        timeout=timeout,
    )
