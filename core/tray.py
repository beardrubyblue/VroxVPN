"""Иконка в системном трее."""
import threading

from PIL import Image, ImageDraw

try:
    import pystray
    PYSTRAY_AVAILABLE = True
except Exception:
    pystray = None
    PYSTRAY_AVAILABLE = False

CONNECTED_COLOR = (76, 175, 80, 255)
DISCONNECTED_COLOR = (136, 136, 136, 255)


def _make_icon_image(connected: bool) -> Image.Image:
    image = Image.new("RGBA", (64, 64), (0, 0, 0, 0))
    draw = ImageDraw.Draw(image)
    color = CONNECTED_COLOR if connected else DISCONNECTED_COLOR
    draw.ellipse((4, 4, 60, 60), fill=color)
    return image


class TrayIcon:
    def __init__(self):
        self.on_show = None
        self.on_toggle = None
        self.on_quit = None
        self.on_select_server = None

        self._connected = False
        self._server_name = ""
        self._selected_server_name = ""
        self._servers = []
        self._icon = None
        self._thread = None

        if PYSTRAY_AVAILABLE:
            self._icon = pystray.Icon(
                "vroxory-vpn",
                icon=_make_icon_image(False),
                title="Vroxory VPN",
                menu=self._build_menu(),
            )

    def _build_servers_menu(self):
        if not self._servers:
            return pystray.Menu(pystray.MenuItem("Нет серверов", None, enabled=False))

        items = []
        for server in self._servers:
            name = server["name"]
            items.append(
                pystray.MenuItem(
                    name,
                    self._make_select_handler(name),
                    checked=self._make_checked_fn(name),
                    radio=True,
                )
            )
        return pystray.Menu(*items)

    def _make_select_handler(self, name: str):
        def handler(_icon, _item):
            self._selected_server_name = name
            if self._icon:
                self._icon.menu = self._build_menu()
            if self.on_select_server:
                self.on_select_server(name)
        return handler

    def _make_checked_fn(self, name: str):
        def fn(_item):
            return self._selected_server_name == name
        return fn

    def _build_menu(self):
        status_text = (
            f"Подключено к: {self._server_name}" if self._connected else "Не подключено"
        )
        toggle_text = "Отключить" if self._connected else "Подключить"
        return pystray.Menu(
            pystray.MenuItem("Vroxory VPN", None, enabled=False),
            pystray.Menu.SEPARATOR,
            pystray.MenuItem(status_text, None, enabled=False),
            pystray.MenuItem("Серверы", self._build_servers_menu()),
            pystray.Menu.SEPARATOR,
            pystray.MenuItem("Показать окно", self._handle_show),
            pystray.MenuItem(toggle_text, self._handle_toggle),
            pystray.Menu.SEPARATOR,
            pystray.MenuItem("Выход", self._handle_quit),
        )

    def _handle_show(self, _icon, _item):
        if self.on_show:
            self.on_show()

    def _handle_toggle(self, _icon, _item):
        if self.on_toggle:
            self.on_toggle()

    def _handle_quit(self, _icon, _item):
        if self.on_quit:
            self.on_quit()

    def start(self) -> None:
        if not self._icon:
            return
        self._thread = threading.Thread(target=self._icon.run, daemon=True)
        self._thread.start()

    def stop(self) -> None:
        if self._icon:
            self._icon.stop()

    def update_status(self, connected: bool, server_name: str = "") -> None:
        self._connected = connected
        self._server_name = server_name
        if server_name:
            self._selected_server_name = server_name
        if not self._icon:
            return
        self._icon.icon = _make_icon_image(connected)
        self._icon.menu = self._build_menu()

    def update_servers(self, servers: list, selected_name: str = "") -> None:
        self._servers = servers
        if selected_name:
            self._selected_server_name = selected_name
        if self._icon:
            self._icon.menu = self._build_menu()
