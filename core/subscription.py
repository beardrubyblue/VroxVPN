"""Загрузка и парсинг подписки hysteria2."""
import base64
import binascii
from urllib.parse import urlparse, parse_qs, unquote

import requests


def fetch_subscription(url: str, timeout: int = 15) -> list:
    """Скачивает подписку по URL и возвращает список серверов."""
    resp = requests.get(url, timeout=timeout)
    resp.raise_for_status()
    text = resp.text.strip()

    if not text.startswith("hysteria2://"):
        text = _try_base64_decode(text)

    servers = []
    for line in text.splitlines():
        line = line.strip()
        if not line.startswith("hysteria2://"):
            continue
        try:
            servers.append(parse_hysteria2_uri(line))
        except ValueError:
            continue
    return servers


def _try_base64_decode(text: str) -> str:
    padded = text + "=" * (-len(text) % 4)
    try:
        decoded = base64.b64decode(padded, validate=False)
        return decoded.decode("utf-8", errors="ignore")
    except (binascii.Error, UnicodeDecodeError):
        return text


def parse_hysteria2_uri(uri: str) -> dict:
    """Парсит одну ссылку hysteria2://[password@]host:port[?params][#name]."""
    if not uri.startswith("hysteria2://"):
        raise ValueError("Not a hysteria2 URI")

    parsed = urlparse(uri)

    password = unquote(parsed.username) if parsed.username else ""
    host = parsed.hostname
    port = parsed.port
    if not host or not port:
        raise ValueError("Missing host or port in hysteria2 URI")

    name = unquote(parsed.fragment) if parsed.fragment else f"{host}:{port}"

    query = parse_qs(parsed.query)

    def first(key, default=""):
        values = query.get(key)
        return values[0] if values else default

    insecure_raw = first("insecure", "0").lower()
    insecure = insecure_raw in ("1", "true", "yes")

    return {
        "name": name,
        "host": host,
        "port": port,
        "password": password,
        "sni": first("sni", host),
        "insecure": insecure,
        "obfs": first("obfs", ""),
        "obfs_password": first("obfs-password", ""),
        "pinSHA256": first("pinSHA256", ""),
        "raw_uri": uri,
    }
