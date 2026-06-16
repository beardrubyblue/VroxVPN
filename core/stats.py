"""Статистика трафика через /proc/net/dev."""
import threading
import time

PROC_NET_DEV = "/proc/net/dev"


def _read_interface_bytes(interface: str):
    """Возвращает (rx_bytes, tx_bytes) для интерфейса или None если не найден."""
    try:
        with open(PROC_NET_DEV, "r", encoding="utf-8") as f:
            lines = f.readlines()
    except OSError:
        return None

    for line in lines[2:]:
        name, _, data = line.partition(":")
        if name.strip() != interface:
            continue
        fields = data.split()
        rx_bytes = int(fields[0])
        tx_bytes = int(fields[8])
        return rx_bytes, tx_bytes

    return None


def format_speed(bps: int) -> str:
    """Форматирует байты/сек в человеко-читаемую строку."""
    if bps < 1024:
        return f"{bps} B/s"
    if bps < 1024 * 1024:
        return f"{round(bps / 1024)} KB/s"
    return f"{bps / (1024 * 1024):.1f} MB/s"


class TrafficStats:
    format_speed = staticmethod(format_speed)

    def __init__(self):
        self.on_update = None
        self._thread = None
        self._stop_event = threading.Event()

    def start(self, interface: str = "tun-vroxory") -> None:
        self._stop_event.clear()
        self._thread = threading.Thread(target=self._run, args=(interface,), daemon=True)
        self._thread.start()

    def stop(self) -> None:
        self._stop_event.set()

    def _run(self, interface: str) -> None:
        previous = _read_interface_bytes(interface)

        while not self._stop_event.is_set():
            time.sleep(1)
            if self._stop_event.is_set():
                break

            current = _read_interface_bytes(interface)
            if current is None or previous is None:
                previous = current
                continue

            rx_prev, tx_prev = previous
            rx_now, tx_now = current
            download_bps = max(rx_now - rx_prev, 0)
            upload_bps = max(tx_now - tx_prev, 0)
            previous = current

            if self.on_update:
                self.on_update(upload_bps, download_bps)
