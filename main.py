#!/usr/bin/env python3
"""Точка входа Vroxory VPN."""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import gi

gi.require_version("Gtk", "4.0")
gi.require_version("Adw", "1")
from gi.repository import Adw, GLib

from ui.main_window import MainWindow
from core.tray import TrayIcon


class VroxoryVPN(Adw.Application):
    def __init__(self):
        super().__init__(application_id="com.vroxory.vpn")
        self.tray = TrayIcon()
        self.tray.on_show = self._on_tray_show
        self.tray.on_toggle = self._on_tray_toggle
        self.tray.on_quit = self._on_tray_quit
        self.tray.on_select_server = self._on_tray_select_server
        self._window = None

    def do_activate(self):
        if self._window is None:
            self._window = MainWindow(self)
            self._window.connect("close-request", self._on_close_request)
            self._window.tun_manager.on_connected = self._wrap(
                self._window.tun_manager.on_connected, self._on_vpn_connected
            )
            self._window.tun_manager.on_disconnected = self._wrap(
                self._window.tun_manager.on_disconnected, self._on_vpn_disconnected
            )
            self._window.on_servers_updated = self._on_servers_updated
            self.tray.start()
        self._window.present()

    def _wrap(self, original, extra):
        def wrapped(*args, **kwargs):
            if original:
                original(*args, **kwargs)
            extra(*args, **kwargs)
        return wrapped

    def _on_close_request(self, _window):
        self._window.hide()
        return True

    def _on_tray_show(self):
        GLib.idle_add(self._window.present)

    def _on_tray_toggle(self):
        GLib.idle_add(self._window._on_connect_clicked, None)

    def _on_vpn_connected(self):
        name = self._window._selected_server["name"] if self._window._selected_server else ""
        self.tray.update_status(True, name)

    def _on_vpn_disconnected(self):
        self.tray.update_status(False)

    def _on_servers_updated(self, servers):
        selected = self._window._selected_server
        selected_name = selected["name"] if selected else ""
        self.tray.update_servers(servers, selected_name)

    def _on_tray_select_server(self, name: str):
        GLib.idle_add(self._window.select_server_by_name, name)

    def _on_tray_quit(self):
        self.request_full_quit()

    def request_full_quit(self):
        """Полностью завершает приложение: отключает VPN (рвёт TUN-
        интерфейс), останавливает трей и закрывает процесс."""
        def worker():
            self._window.tun_manager.disconnect()
            GLib.idle_add(self._finish_quit)

        import threading
        threading.Thread(target=worker, daemon=True).start()

    def _finish_quit(self):
        self.tray.stop()
        self.quit()


def main():
    app = VroxoryVPN()
    return app.run(sys.argv)


if __name__ == "__main__":
    sys.exit(main())
