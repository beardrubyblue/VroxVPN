"""Пинг серверов."""
import re
import subprocess
from concurrent.futures import ThreadPoolExecutor

PING_RE = re.compile(r"time[=<]([\d.]+)\s*ms")


def ping_server(host: str, port: int = None, timeout: float = 3.0) -> int | None:
    """ICMP-пинг host, возвращает задержку в мс или None если недоступен.

    hysteria2 работает по UDP/QUIC, поэтому TCP connect к порту сервера
    обычно не проходит даже при живом сервере — используем системный ping.
    """
    wait_seconds = max(1, int(timeout))
    try:
        result = subprocess.run(
            ["ping", "-c", "1", "-W", str(wait_seconds), host],
            capture_output=True,
            text=True,
            timeout=timeout + 2,
        )
    except (subprocess.TimeoutExpired, OSError):
        return None

    if result.returncode != 0:
        return None

    match = PING_RE.search(result.stdout)
    return round(float(match.group(1))) if match else None


def ping_all_servers(servers: list, callback, max_workers: int = 10) -> None:
    """Пингует все серверы параллельно, вызывает callback(name, latency_ms) по готовности."""
    def worker(server):
        latency = ping_server(server["host"], server["port"])
        callback(server["name"], latency)

    with ThreadPoolExecutor(max_workers=max_workers) as executor:
        executor.map(worker, servers)
