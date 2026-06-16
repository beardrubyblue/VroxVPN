"""Установка бинарника hysteria2."""
import os
import platform
import stat
from pathlib import Path

import requests

GITHUB_API_LATEST = "https://api.github.com/repos/apernet/hysteria/releases/latest"

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


def download_hysteria2(progress_callback=None) -> Path:
    """Скачивает последний релиз hysteria2 для текущей архитектуры.

    progress_callback(downloaded_bytes, total_bytes) вызывается по ходу загрузки.
    """
    arch = _target_arch()
    asset_name = f"hysteria-linux-{arch}"

    resp = requests.get(GITHUB_API_LATEST, timeout=15)
    resp.raise_for_status()
    release = resp.json()

    asset_url = None
    for asset in release.get("assets", []):
        if asset.get("name") == asset_name:
            asset_url = asset.get("browser_download_url")
            break

    if not asset_url:
        raise RuntimeError(f"Asset {asset_name} не найден в последнем релизе hysteria2")

    dest = Path(get_binary_path())
    dest.parent.mkdir(parents=True, exist_ok=True)

    with requests.get(asset_url, stream=True, timeout=30) as r:
        r.raise_for_status()
        total = int(r.headers.get("Content-Length", 0))
        downloaded = 0
        tmp_path = dest.with_suffix(".tmp")
        with open(tmp_path, "wb") as f:
            for chunk in r.iter_content(chunk_size=8192):
                if not chunk:
                    continue
                f.write(chunk)
                downloaded += len(chunk)
                if progress_callback:
                    progress_callback(downloaded, total)
        tmp_path.replace(dest)

    dest.chmod(dest.stat().st_mode | stat.S_IEXEC | stat.S_IXGRP | stat.S_IXOTH)
    return dest
