"""Автозапуск приложения при входе в систему через XDG autostart."""
from pathlib import Path

AUTOSTART_DIR = Path.home() / ".config" / "autostart"
DESKTOP_PATH = AUTOSTART_DIR / "com.vroxory.vpn.desktop"

# NoDisplay + --minimized: при автозапуске окно не всплывает, приложение
# сразу уходит в трей (см. main.py: VroxoryVPN.do_activate)
DESKTOP_CONTENT = """[Desktop Entry]
Type=Application
Name=vrox.vpn
Comment=Hysteria2 VPN клиент
Exec=vroxory-vpn --minimized
Icon=com.vroxory.vpn
Terminal=false
NoDisplay=true
X-GNOME-Autostart-enabled=true
"""


def is_enabled() -> bool:
    return DESKTOP_PATH.exists()


def enable() -> None:
    AUTOSTART_DIR.mkdir(parents=True, exist_ok=True)
    DESKTOP_PATH.write_text(DESKTOP_CONTENT, encoding="utf-8")


def disable() -> None:
    DESKTOP_PATH.unlink(missing_ok=True)
