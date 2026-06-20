#!/usr/bin/env python3
"""Точка входа vrox.vpn."""
import sys
import threading
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import gi

gi.require_version("Gtk", "4.0")
gi.require_version("Adw", "1")
from gi.repository import Adw, GLib

from ui.main_window import MainWindow
from core.tray import TrayIcon
from core.procname import set_process_name

set_process_name("vrox.vpn")

# Сколько максимум ждём корректное отключение VPN перед выходом. pkexec
# при очистке TUN/DNS иногда отвечает не мгновенно (несколько секунд) —
# без жёсткого потолка зависший вызов держал бы открытыми и окно, и
# иконку в трее бесконечно после клика "Выход".
QUIT_TIMEOUT_SECONDS = 8


class VroxoryVPN(Adw.Application):
    def __init__(self, start_minimized: bool = False):
        super().__init__(application_id="com.vroxory.vpn")
        self._start_minimized = start_minimized
        self.tray = TrayIcon()
        self.tray.on_show = self._on_tray_show
        self.tray.on_toggle = self._on_tray_toggle
        self.tray.on_quit = self._on_tray_quit
        self.tray.on_select_server = self._on_tray_select_server
        self._window = None
        self._quit_requested = False
        self._quit_finished = False

    def do_activate(self):
        first_activation = self._window is None
        if first_activation:
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

        # автозапуск (--minimized): окно сразу уходит в трей, не мигая
        # на экране при каждом входе в систему
        if first_activation and self._start_minimized:
            return
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
        интерфейс), останавливает трей и закрывает процесс.

        disconnect() сам по себе уже не блокируется бесконечно (см.
        run_privileged), но на случай ЛЮБОЙ другой неожиданной задержки
        ставим запасной таймер — приложение обязано закрыться не позже
        QUIT_TIMEOUT_SECONDS после клика "Выход", даже если что-то
        зависнет."""
        if self._quit_requested:
            return
        self._quit_requested = True

        def worker():
            try:
                self._window.tun_manager.disconnect()
            except Exception as exc:
                print(f"[quit] ошибка при отключении: {exc}")
            GLib.idle_add(self._finish_quit)

        threading.Thread(target=worker, daemon=True).start()
        GLib.timeout_add_seconds(QUIT_TIMEOUT_SECONDS, self._finish_quit)

    def _finish_quit(self):
        if self._quit_finished:
            return False
        self._quit_finished = True
        self.tray.stop()
        self.quit()
        return False


def main():
    GLib.set_application_name("vrox.vpn")
    # GApplication.run() сам парсит argv и падает на незнакомых опциях —
    # поэтому --minimized убираем из argv до передачи, а не оставляем
    # GLib'у разбираться с ним
    argv = sys.argv
    start_minimized = "--minimized" in argv
    if start_minimized:
        argv = [a for a in argv if a != "--minimized"]
    app = VroxoryVPN(start_minimized=start_minimized)
    return app.run(argv)


if __name__ == "__main__":
    sys.exit(main())
