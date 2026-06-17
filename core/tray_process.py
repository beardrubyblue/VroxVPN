#!/usr/bin/env python3
"""Отдельный процесс для иконки в системном трее.

PyGObject не допускает смешивание GTK3 и GTK4 в одном процессе, а
AyatanaAppIndicator3 (без него GNOME Shell не показывает трей-иконки)
работает только через GTK3. Поэтому главное окно (GTK4) общается с этим
процессом (GTK3) через простой текстовый протокол по stdin/stdout —
по строке на команду/событие.

Протокол (родитель -> сюда, через stdin):
  STATUS:<0|1>:<server_name>   — обновить статус подключения
  SERVERS:<name1>\x1f<name2>… — обновить список серверов (разделитель \x1f)
  SELECTED:<name>               — отметить выбранный сервер в подменю
  QUIT_PROCESS                  — завершить процесс трея

Протокол (сюда -> родителю, через stdout):
  SHOW           — клик "Показать окно"
  TOGGLE         — клик "Подключить"/"Отключить"
  QUIT           — клик "Выход"
  SELECT:<name>  — выбран сервер в подменю
"""
import ctypes
import sys
import tempfile
import threading
from pathlib import Path

try:
    ctypes.CDLL(None).prctl(15, b"vrox.vpn", 0, 0, 0)  # PR_SET_NAME — иначе в системном
    # мониторе/ps этот процесс виден как голый "python3", а не "vrox.vpn"
except (OSError, AttributeError):
    pass

import gi

gi.require_version("Gtk", "3.0")
gi.require_version("AyatanaAppIndicator3", "0.1")
from gi.repository import Gtk, AyatanaAppIndicator3 as AppIndicator3, GLib

from PIL import Image, ImageDraw

ICON_DIR = Path(tempfile.gettempdir()) / "vroxory-vpn"
CONNECTED_COLOR = (76, 175, 80, 255)
DISCONNECTED_COLOR = (136, 136, 136, 255)


def _make_icon_path(connected: bool) -> str:
    ICON_DIR.mkdir(parents=True, exist_ok=True)
    path = ICON_DIR / ("tray-on.png" if connected else "tray-off.png")
    if not path.exists():
        image = Image.new("RGBA", (64, 64), (0, 0, 0, 0))
        draw = ImageDraw.Draw(image)
        color = CONNECTED_COLOR if connected else DISCONNECTED_COLOR
        draw.ellipse((4, 4, 60, 60), fill=color)
        image.save(path)
    return str(path)


class TrayProcess:
    def __init__(self):
        self.connected = False
        self.server_name = ""
        self.selected_server = ""
        self.servers = []

        self.indicator = AppIndicator3.Indicator.new(
            "com.vroxory.vpn",
            _make_icon_path(False),
            AppIndicator3.IndicatorCategory.APPLICATION_STATUS,
        )
        self.indicator.set_status(AppIndicator3.IndicatorStatus.ACTIVE)
        self.indicator.set_title("vrox.vpn")
        self._rebuild_menu()

        threading.Thread(target=self._read_stdin, daemon=True).start()

    def _rebuild_menu(self):
        menu = Gtk.Menu()

        status_text = f"Подключено к: {self.server_name}" if self.connected else "Не подключено"
        status_item = Gtk.MenuItem(label=status_text)
        status_item.set_sensitive(False)
        menu.append(status_item)

        menu.append(Gtk.SeparatorMenuItem())

        servers_item = Gtk.MenuItem(label="Серверы")
        submenu = Gtk.Menu()
        if self.servers:
            group_owner = None
            for name in self.servers:
                item = Gtk.RadioMenuItem.new_with_label(
                    group_owner.get_group() if group_owner else [], name
                )
                if group_owner is None:
                    group_owner = item
                item.set_active(name == self.selected_server)
                item.connect("toggled", self._on_select_toggled, name)
                submenu.append(item)
        else:
            none_item = Gtk.MenuItem(label="Нет серверов")
            none_item.set_sensitive(False)
            submenu.append(none_item)
        servers_item.set_submenu(submenu)
        menu.append(servers_item)

        menu.append(Gtk.SeparatorMenuItem())

        show_item = Gtk.MenuItem(label="Показать окно")
        show_item.connect("activate", lambda _i: self._send("SHOW"))
        menu.append(show_item)

        toggle_item = Gtk.MenuItem(label="Отключить" if self.connected else "Подключить")
        toggle_item.connect("activate", lambda _i: self._send("TOGGLE"))
        menu.append(toggle_item)

        menu.append(Gtk.SeparatorMenuItem())

        quit_item = Gtk.MenuItem(label="Выход")
        quit_item.connect("activate", lambda _i: self._send("QUIT"))
        menu.append(quit_item)

        menu.show_all()
        self.indicator.set_menu(menu)

    def _on_select_toggled(self, item, name):
        if item.get_active():
            self.selected_server = name
            self._send(f"SELECT:{name}")

    def _send(self, line: str) -> None:
        print(line, flush=True)

    def _read_stdin(self) -> None:
        for raw in sys.stdin:
            line = raw.strip()
            if line:
                GLib.idle_add(self._handle_command, line)

    def _handle_command(self, line: str):
        if line == "QUIT_PROCESS":
            Gtk.main_quit()
            return False

        if line.startswith("STATUS:"):
            _, connected_str, name = line.split(":", 2)
            self.connected = connected_str == "1"
            self.server_name = name
            if name:
                self.selected_server = name
            self.indicator.set_icon_full(_make_icon_path(self.connected), "vroxory-vpn")
            self._rebuild_menu()
        elif line.startswith("SERVERS:"):
            payload = line[len("SERVERS:"):]
            self.servers = payload.split("\x1f") if payload else []
            self._rebuild_menu()
        elif line.startswith("SELECTED:"):
            self.selected_server = line[len("SELECTED:"):]
            self._rebuild_menu()
        return False


def main() -> None:
    TrayProcess()
    Gtk.main()


if __name__ == "__main__":
    main()
