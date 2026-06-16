"""Строка списка серверов."""
import gi

gi.require_version("Gtk", "4.0")
gi.require_version("Adw", "1")
from gi.repository import Gtk, Adw


class ServerRow(Adw.ActionRow):
    def __init__(self, server: dict):
        super().__init__()
        self.server = server

        self.set_title(server["name"])
        self.set_subtitle(f"{server['host']}:{server['port']}")
        self.set_activatable(True)
        self.add_css_class("activatable")

        icon = Gtk.Image.new_from_icon_name("network-server-symbolic")
        icon.set_pixel_size(20)
        self.add_prefix(icon)

        self.ping_label = Gtk.Label(label="—")
        self.ping_label.add_css_class("caption")
        self.ping_label.add_css_class("dim-label")
        self.add_suffix(self.ping_label)

        arrow = Gtk.Image.new_from_icon_name("go-next-symbolic")
        arrow.add_css_class("dim-label")
        self.add_suffix(arrow)

    def set_ping(self, latency_ms: int | None) -> None:
        for cls in ("success", "ping-warn", "error", "dim-label"):
            self.ping_label.remove_css_class(cls)

        if latency_ms is None:
            self.ping_label.set_label("—")
            self.ping_label.add_css_class("dim-label")
        elif latency_ms < 100:
            self.ping_label.set_label(f"{latency_ms} ms")
            self.ping_label.add_css_class("success")
        elif latency_ms <= 300:
            self.ping_label.set_label(f"{latency_ms} ms")
            self.ping_label.add_css_class("ping-warn")
        else:
            self.ping_label.set_label(f"{latency_ms} ms")
            self.ping_label.add_css_class("error")
