"""Карточка со скоростью трафика — нативный Adw.PreferencesGroup."""
import gi

gi.require_version("Gtk", "4.0")
gi.require_version("Adw", "1")
from gi.repository import Gtk, Adw

from core.stats import format_speed


class StatsBar(Adw.PreferencesGroup):
    def __init__(self):
        super().__init__()
        self.set_visible(False)

        row = Adw.ActionRow()
        row.set_title("Трафик")
        row.set_subtitle("↑ 0 B/s · ↓ 0 B/s")

        icon = Gtk.Image.new_from_icon_name("network-transmit-receive-symbolic")
        row.add_prefix(icon)

        self.upload_label = Gtk.Label(label="↑ 0 B/s")
        self.upload_label.add_css_class("success")
        row.add_suffix(self.upload_label)

        self.download_label = Gtk.Label(label="↓ 0 B/s")
        self.download_label.add_css_class("accent")
        row.add_suffix(self.download_label)

        self._row = row
        self.add(row)

    def update(self, upload_bps: int, download_bps: int) -> None:
        up = format_speed(upload_bps)
        down = format_speed(download_bps)
        self.upload_label.set_label(f"↑ {up}")
        self.download_label.set_label(f"↓ {down}")
        self._row.set_subtitle(f"↑ {up} · ↓ {down}")
