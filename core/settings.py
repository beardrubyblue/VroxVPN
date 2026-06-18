"""Хранение настроек приложения в JSON файле."""
import json
from pathlib import Path

SETTINGS_DIR = Path.home() / ".config" / "vroxory-vpn"
SETTINGS_PATH = SETTINGS_DIR / "settings.json"

DEFAULTS = {
    "subscription_url": "",
    "last_selected_server": "",
    "auto_reconnect": False,
    "ru_bypass_enabled": False,
}


def load() -> dict:
    if not SETTINGS_PATH.exists():
        return dict(DEFAULTS)
    try:
        with open(SETTINGS_PATH, "r", encoding="utf-8") as f:
            data = json.load(f)
    except (json.JSONDecodeError, OSError):
        return dict(DEFAULTS)
    merged = dict(DEFAULTS)
    merged.update(data)
    return merged


def save(data: dict) -> None:
    SETTINGS_DIR.mkdir(parents=True, exist_ok=True)
    with open(SETTINGS_PATH, "w", encoding="utf-8") as f:
        json.dump(data, f, ensure_ascii=False, indent=2)


def get(key: str, default=None):
    return load().get(key, default)


def set(key: str, value) -> None:
    data = load()
    data[key] = value
    save(data)
