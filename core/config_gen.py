"""Генерация YAML конфига hysteria2 для TUN-режима."""
import re
import socket
from pathlib import Path

import yaml

from core import geoip, settings

CONFIG_DIR = Path("/tmp/vroxory-vpn")


def _safe_filename(name: str) -> str:
    return re.sub(r"[^a-zA-Z0-9_-]+", "_", name).strip("_") or "server"


def _resolve_server_addresses(host: str) -> tuple[list[str], list[str]]:
    """Резолвит host в IPv4/IPv6 адреса — нужны для exclude-маршрутов,
    иначе пакеты к самому VPN-серверу уйдут в TUN и получится routing loop."""
    ipv4_addrs, ipv6_addrs = [], []
    try:
        for family, _, _, _, sockaddr in socket.getaddrinfo(host, None):
            ip = sockaddr[0]
            if family == socket.AF_INET and ip not in ipv4_addrs:
                ipv4_addrs.append(ip)
            elif family == socket.AF_INET6 and ip not in ipv6_addrs:
                ipv6_addrs.append(ip)
    except socket.gaierror:
        pass
    return ipv4_addrs, ipv6_addrs


def generate_config(server: dict) -> Path:
    """Генерирует YAML конфиг для сервера и возвращает путь к файлу."""
    CONFIG_DIR.mkdir(parents=True, exist_ok=True)

    server_ipv4, server_ipv6 = _resolve_server_addresses(server["host"])

    ipv4_exclude = [
        "192.168.0.0/16",
        "10.0.0.0/8",
        "172.16.0.0/12",
        "127.0.0.0/8",
    ] + [f"{ip}/32" for ip in server_ipv4]
    ipv6_exclude = ["fc00::/7", "fe80::/10"] + [f"{ip}/128" for ip in server_ipv6]

    if settings.get("ru_bypass_enabled", False):
        ru_ipv4, ru_ipv6 = geoip.get_ru_cidrs()
        ipv4_exclude += ru_ipv4
        ipv6_exclude += ru_ipv6

    # socks5/http секции omitted намеренно: hysteria2 не поддерживает их "disable",
    # присутствие ключа само запускает сервер — отсутствие ключа отключает его.
    config = {
        "server": f"{server['host']}:{server['port']}",
        "auth": server.get("password", ""),
        "tls": {
            "sni": server.get("sni") or server["host"],
        },
        "tun": {
            "name": "tun-vroxory",
            "mtu": 1500,
            "timeout": 300,
            "address": {
                "ipv4": "100.100.100.101/30",
                "ipv6": "2001::ffff:ffff:ffff:fff1/126",
            },
            "route": {
                "ipv4": ["0.0.0.0/0"],
                "ipv6": ["::/0"],
                "ipv4Exclude": ipv4_exclude,
                "ipv6Exclude": ipv6_exclude,
            },
        },
        "fastOpen": True,
        "bandwidth": {"up": "100 mbps", "down": "100 mbps"},
    }

    if server.get("insecure"):
        config["tls"]["insecure"] = True

    if server.get("pinSHA256"):
        config["tls"]["pinSHA256"] = server["pinSHA256"]

    if server.get("obfs"):
        config["obfs"] = {
            "type": server["obfs"],
            "salamander": {"password": server.get("obfs_password", "")},
        }

    if server.get("quic"):
        config["quic"] = server["quic"]

    filename = f"{_safe_filename(server.get('name', 'server'))}.yaml"
    path = CONFIG_DIR / filename
    with open(path, "w", encoding="utf-8") as f:
        yaml.safe_dump(config, f, allow_unicode=True, sort_keys=False)

    return path
