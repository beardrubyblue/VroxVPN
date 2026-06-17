"""Загрузка и парсинг подписки hysteria2."""
import base64
import binascii
import json
from urllib.parse import urlparse, parse_qs, unquote

import requests

# 3x-ui кладёт QUIC-тюнинг в quicParams (формат xray-core "finalmask"), но
# часть имён полей у бинарника hysteria2 называется иначе — маппинг ниже.
# Поля congestion/bbrProfile/debug/maxIncomingStreams/udpHop не имеют
# соответствия в клиентском конфиге hysteria2 (xray-core-only либо
# серверные) и сюда не попадают.
QUIC_FIELD_MAP = {
    "initStreamReceiveWindow": "initStreamReceiveWindow",
    "maxStreamReceiveWindow": "maxStreamReceiveWindow",
    "initConnectionReceiveWindow": "initConnReceiveWindow",
    "maxConnectionReceiveWindow": "maxConnReceiveWindow",
    "maxIdleTimeout": "maxIdleTimeout",
    "keepAlivePeriod": "keepAlivePeriod",
    "disablePathMTUDiscovery": "disablePathMTUDiscovery",
}


def fetch_subscription(url: str, timeout: int = 15) -> tuple:
    """Скачивает подписку по URL. Возвращает (список серверов, userinfo).

    userinfo — это разобранный заголовок Subscription-Userinfo
    (upload/download/total/expire в байтах/unix-time), который 3x-ui
    отдаёт на сам запрос подписки — используется клиентами вроде Clash
    для показа "сколько трафика осталось/когда истекает". Пустой dict,
    если сервер заголовок не присылает.
    """
    resp = requests.get(url, timeout=timeout)
    resp.raise_for_status()
    text = resp.text.strip()

    userinfo = _parse_userinfo(resp.headers.get("Subscription-Userinfo", ""))

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
    return servers, userinfo


def _parse_userinfo(header_value: str) -> dict:
    info = {}
    for part in header_value.split(";"):
        part = part.strip()
        if "=" not in part:
            continue
        key, _, value = part.partition("=")
        try:
            info[key.strip()] = int(value.strip())
        except ValueError:
            continue
    return info


def _parse_quic_params(fm_raw: str) -> dict:
    """Парсит query-параметр fm (JSON finalmask от 3x-ui) и возвращает
    только те поля quicParams, у которых есть соответствие в секции
    quic: клиентского конфига hysteria2 (см. QUIC_FIELD_MAP)."""
    if not fm_raw:
        return {}
    try:
        finalmask = json.loads(fm_raw)
    except (json.JSONDecodeError, TypeError):
        return {}
    quic_params = finalmask.get("quicParams")
    if not isinstance(quic_params, dict):
        return {}
    return {
        hysteria_key: quic_params[sub_key]
        for sub_key, hysteria_key in QUIC_FIELD_MAP.items()
        if sub_key in quic_params
    }


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
        "quic": _parse_quic_params(first("fm", "")),
        "raw_uri": uri,
    }
