"""Установка бинарника hysteria2 — из НАШЕГО форка apernet/hysteria
(см. packaging/hysteria2-patch/), а не из оригинальных релизов apernet:
форк добавляет directDomains (обход VPN по списку доменов через
DNS-сниффинг на реальном интерфейсе), которого в оригинале нет.

Тег релиза фиксированный, не "latest" — хотим явно контролировать,
какая версия апстрима/патча ставится пользователям, а не подхватывать
произвольный новый релиз форка автоматически."""
import hashlib
import os
import platform
import stat
from pathlib import Path

import requests

GITHUB_API_RELEASE = "https://api.github.com/repos/beardrubyblue/VroxVPN/releases/tags/hysteria2-fork-v2.9.2-1"

ARCH_MAP = {
    "x86_64": "amd64",
    "amd64": "amd64",
    "aarch64": "arm64",
    "arm64": "arm64",
}


def _target_arch() -> str:
    machine = platform.machine()
    arch = ARCH_MAP.get(machine)
    if not arch:
        raise RuntimeError(f"Неподдерживаемая архитектура: {machine}")
    return arch


def get_binary_path() -> str:
    if os.geteuid() == 0:
        return "/usr/local/bin/hysteria2"
    return str(Path.home() / ".local" / "bin" / "hysteria2")


def is_installed() -> bool:
    path = Path(get_binary_path())
    return path.exists() and os.access(path, os.X_OK)


def _fetch_expected_hash(release: dict, asset_name: str) -> str:
    """Ищет sha256 для asset_name в hashes.txt релиза (наш hashes.txt,
    публикуется packaging/hysteria2-patch/build.sh — формат sha256sum:
    "<hash>  <filename>"). Возвращает пустую строку, если hashes.txt
    почему-то недоступен — это не должно блокировать установку."""
    hashes_url = None
    for asset in release.get("assets", []):
        if asset.get("name") == "hashes.txt":
            hashes_url = asset.get("browser_download_url")
            break

    if not hashes_url:
        return ""

    try:
        resp = requests.get(hashes_url, timeout=10)
        resp.raise_for_status()
    except requests.RequestException:
        return ""

    for line in resp.text.splitlines():
        parts = line.split()
        if len(parts) == 2 and parts[1] == asset_name:
            return parts[0].lower()
    return ""


def download_hysteria2(progress_callback=None) -> Path:
    """Скачивает наш форк hysteria2 (фиксированный релиз-тег, см. модуль)
    для текущей архитектуры.

    Перед заменой текущего бинарника проверяет sha256 против hashes.txt
    в том же релизе (если найден) — без этого скачанный с GitHub файл
    устанавливался бы без какой-либо проверки целостности.

    progress_callback(downloaded_bytes, total_bytes) вызывается по ходу загрузки.
    """
    arch = _target_arch()
    asset_name = f"hysteria2-vroxory-linux-{arch}"

    resp = requests.get(GITHUB_API_RELEASE, timeout=15)
    resp.raise_for_status()
    release = resp.json()

    asset_url = None
    for asset in release.get("assets", []):
        if asset.get("name") == asset_name:
            asset_url = asset.get("browser_download_url")
            break

    if not asset_url:
        raise RuntimeError(f"Asset {asset_name} не найден в последнем релизе hysteria2")

    expected_hash = _fetch_expected_hash(release, asset_name)

    dest = Path(get_binary_path())
    dest.parent.mkdir(parents=True, exist_ok=True)
    tmp_path = dest.with_suffix(".tmp")

    digest = hashlib.sha256()
    with requests.get(asset_url, stream=True, timeout=30) as r:
        r.raise_for_status()
        total = int(r.headers.get("Content-Length", 0))
        downloaded = 0
        with open(tmp_path, "wb") as f:
            for chunk in r.iter_content(chunk_size=8192):
                if not chunk:
                    continue
                f.write(chunk)
                digest.update(chunk)
                downloaded += len(chunk)
                if progress_callback:
                    progress_callback(downloaded, total)

    if expected_hash and digest.hexdigest().lower() != expected_hash:
        tmp_path.unlink(missing_ok=True)
        raise RuntimeError(
            "Проверка sha256 hysteria2 не пройдена — скачанный файл повреждён "
            "или подменён, установка отменена"
        )

    tmp_path.replace(dest)
    dest.chmod(dest.stat().st_mode | stat.S_IEXEC | stat.S_IXGRP | stat.S_IXOTH)
    return dest
