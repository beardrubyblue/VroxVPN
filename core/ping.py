"""TCP-пинг серверов."""
import socket
import time
from concurrent.futures import ThreadPoolExecutor


def ping_server(host: str, port: int, timeout: float = 3.0) -> int | None:
    """TCP connect к host:port, возвращает задержку в мс или None если недоступен."""
    start = time.monotonic()
    try:
        with socket.create_connection((host, port), timeout=timeout):
            pass
    except OSError:
        return None
    return round((time.monotonic() - start) * 1000)


def ping_all_servers(servers: list, callback, max_workers: int = 10) -> None:
    """Пингует все серверы параллельно, вызывает callback(name, latency_ms) по готовности."""
    def worker(server):
        latency = ping_server(server["host"], server["port"])
        callback(server["name"], latency)

    with ThreadPoolExecutor(max_workers=max_workers) as executor:
        executor.map(worker, servers)
