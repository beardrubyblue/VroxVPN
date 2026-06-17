"""Имя процесса в системном мониторе/ps/top.

Без этого оба процесса (главный GTK4 и отдельный GTK3-процесс трея,
core/tray_process.py) видны как голый "python3" — интерпретатор, под
которым они запущены, а не имя приложения."""
import ctypes

PR_SET_NAME = 15


def set_process_name(name: str) -> None:
    try:
        libc = ctypes.CDLL(None)
        libc.prctl(PR_SET_NAME, name.encode("utf-8"), 0, 0, 0)
    except (OSError, AttributeError):
        pass
