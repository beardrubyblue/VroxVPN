"""CIDR-диапазоны IP-адресов России для обхода VPN (geoip-bypass).

Источник — ipverse/country-ip-blocks (агрегированные данные RIPE NCC
allocation), обновляется независимо от приложения. С приложением
поставляется снимок на момент релиза (data/geoip/), а через "Обновить"
в настройках можно скачать актуальную версию — она сохраняется отдельно
и имеет приоритет над встроенным снимком.
"""
from pathlib import Path

import requests

BUNDLED_DIR = Path(__file__).resolve().parent.parent / "data" / "geoip"
USER_DIR = Path.home() / ".config" / "vroxory-vpn" / "geoip"

SOURCE_URLS = {
    "ipv4": "https://raw.githubusercontent.com/ipverse/country-ip-blocks/master/country/ru/ipv4-aggregated.txt",
    "ipv6": "https://raw.githubusercontent.com/ipverse/country-ip-blocks/master/country/ru/ipv6-aggregated.txt",
}


def _parse_cidr_file(path: Path) -> list:
    if not path.exists():
        return []
    with open(path, "r", encoding="utf-8") as f:
        return [line.strip() for line in f if line.strip() and not line.startswith("#")]


def get_ru_cidrs() -> tuple:
    """Возвращает (ipv4_cidrs, ipv6_cidrs). Файлы из USER_DIR (после
    "Обновить") приоритетнее встроенного снимка из BUNDLED_DIR."""
    ipv4_path = USER_DIR / "ru_ipv4.txt"
    ipv6_path = USER_DIR / "ru_ipv6.txt"
    if not ipv4_path.exists():
        ipv4_path = BUNDLED_DIR / "ru_ipv4.txt"
    if not ipv6_path.exists():
        ipv6_path = BUNDLED_DIR / "ru_ipv6.txt"
    return _parse_cidr_file(ipv4_path), _parse_cidr_file(ipv6_path)


def last_updated() -> str:
    """Дата снимка, который сейчас используется — обновлённого пользователем
    или встроенного в приложение."""
    user_file = USER_DIR / "ru_ipv4.txt"
    if user_file.exists():
        import datetime
        return datetime.datetime.fromtimestamp(user_file.stat().st_mtime).strftime("%d.%m.%Y %H:%M")
    return "встроенная база (из установки)"


def update_ru_cidrs(timeout: int = 20) -> int:
    """Скачивает свежие списки и сохраняет в USER_DIR. Возвращает суммарное
    количество CIDR-диапазонов. Бросает исключение при сетевой ошибке —
    вызывающий код должен показать его пользователю."""
    USER_DIR.mkdir(parents=True, exist_ok=True)
    total = 0
    for key, filename in (("ipv4", "ru_ipv4.txt"), ("ipv6", "ru_ipv6.txt")):
        resp = requests.get(SOURCE_URLS[key], timeout=timeout)
        resp.raise_for_status()
        text = resp.text
        cidrs = [line.strip() for line in text.splitlines() if line.strip() and not line.startswith("#")]
        if not cidrs:
            raise ValueError(f"Пустой ответ при обновлении базы {key}")
        with open(USER_DIR / filename, "w", encoding="utf-8") as f:
            f.write(text)
        total += len(cidrs)
    return total
