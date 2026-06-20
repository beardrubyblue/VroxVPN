"""Список доменов российских сервисов для обхода VPN (geosite-bypass).

Источник — v2fly/domain-list-community, файл "category-ru": кураторский
список (банки, госуслуги, ритейл, медиа, IT-компании и т.д.), устроенный
через рекурсивные "include:" на другие файлы того же репозитория — при
обновлении разворачиваем их в плоский список доменов.

Этот список нужен в дополнение к geoip (core/geoip.py): geoip исключает
по IP-диапазонам России, но сайты, которые физически хостятся на
зарубежном CDN (Cloudflare и т.п.), под него не попадают — для них и
нужен список доменов, передаваемый в наш патч hysteria2 (directDomains),
который сам узнаёт текущий IP через DNS-сниффинг.
"""
import concurrent.futures
from pathlib import Path

import requests

BUNDLED_DIR = Path(__file__).resolve().parent.parent / "data" / "geosite"
USER_DIR = Path.home() / ".config" / "vroxory-vpn" / "geosite"

SOURCE_BASE = "https://raw.githubusercontent.com/v2fly/domain-list-community/master/data/"
ROOT_CATEGORY = "category-ru"


def _parse_domain_file(path: Path) -> list:
    if not path.exists():
        return []
    with open(path, "r", encoding="utf-8") as f:
        return [line.strip() for line in f if line.strip() and not line.startswith("#")]


def get_ru_domains() -> list:
    """Возвращает список доменов. Файл из USER_DIR (после "Обновить")
    приоритетнее встроенного снимка из BUNDLED_DIR."""
    path = USER_DIR / "ru_domains.txt"
    if not path.exists():
        path = BUNDLED_DIR / "ru_domains.txt"
    return _parse_domain_file(path)


def last_updated() -> str:
    user_file = USER_DIR / "ru_domains.txt"
    if user_file.exists():
        import datetime
        return datetime.datetime.fromtimestamp(user_file.stat().st_mtime).strftime("%d.%m.%Y %H:%M")
    return "встроенный список (из установки)"


def current_size_kb() -> float:
    path = USER_DIR / "ru_domains.txt"
    if not path.exists():
        path = BUNDLED_DIR / "ru_domains.txt"
    return path.stat().st_size / 1024 if path.exists() else 0.0


def _fetch_file(name: str, timeout: int, retries: int = 2) -> str | None:
    for _ in range(retries):
        try:
            resp = requests.get(SOURCE_BASE + name, timeout=timeout)
            resp.raise_for_status()
            return resp.text
        except requests.RequestException:
            continue
    return None


def update_ru_domains(timeout: int = 15) -> dict:
    """Рекурсивно разворачивает category-ru (и все include: внутри) в
    плоский список доменов и сохраняет в USER_DIR. Возвращает
    {"count": кол-во доменов, "bytes": размер сохранённого файла}.
    Файлы, которые не удалось скачать (например, переименованные в
    апстриме), просто пропускаются — список всё равно получается
    рабочим, просто чуть менее полным."""
    seen_files = set()
    domains = set()
    pending = {ROOT_CATEGORY}

    with concurrent.futures.ThreadPoolExecutor(max_workers=24) as pool:
        while pending:
            to_fetch = [n for n in pending if n not in seen_files]
            seen_files.update(to_fetch)
            pending = set()
            if not to_fetch:
                break
            fetched = pool.map(lambda name: (name, _fetch_file(name, timeout)), to_fetch)
            for name, text in fetched:
                if text is None:
                    continue
                for line in text.splitlines():
                    line = line.strip()
                    if not line or line.startswith("#"):
                        continue
                    line = line.split("#")[0].strip()
                    if not line:
                        continue
                    if line.startswith("include:"):
                        inc = line[len("include:"):].strip()
                        if inc not in seen_files:
                            pending.add(inc)
                        continue
                    entry = line.split("@")[0].strip()
                    for prefix in ("full:", "domain:"):
                        if entry.startswith(prefix):
                            entry = entry[len(prefix):]
                            break
                    if entry.startswith(("regexp:", "keyword:")) or not entry:
                        continue
                    domains.add(entry.lower())

    if not domains:
        raise ValueError("Не удалось скачать ни одного домена из category-ru")

    USER_DIR.mkdir(parents=True, exist_ok=True)
    text = "\n".join(sorted(domains)) + "\n"
    path = USER_DIR / "ru_domains.txt"
    with open(path, "w", encoding="utf-8") as f:
        f.write(text)

    return {"count": len(domains), "bytes": len(text.encode("utf-8"))}
