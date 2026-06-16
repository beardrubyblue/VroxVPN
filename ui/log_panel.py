"""Панель логов с цветовой маркировкой по уровню."""
import time

import gi

gi.require_version("Gtk", "4.0")
from gi.repository import Gtk

MAX_LINES = 500


class LogPanel(Gtk.Box):
    def __init__(self):
        super().__init__(orientation=Gtk.Orientation.VERTICAL, spacing=4)
        self.set_margin_top(8)
        self.set_margin_bottom(8)
        self.set_margin_start(8)
        self.set_margin_end(8)
        self.set_vexpand(True)

        header = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=8)

        title = Gtk.Label(label="Логи")
        title.add_css_class("heading")
        title.set_halign(Gtk.Align.START)
        title.set_hexpand(True)
        header.append(title)

        copy_btn = Gtk.Button(label="Копировать")
        copy_btn.connect("clicked", lambda _b: self.copy_to_clipboard())
        header.append(copy_btn)

        clear_btn = Gtk.Button(label="Очистить")
        clear_btn.connect("clicked", lambda _b: self.clear())
        header.append(clear_btn)

        self.append(header)

        scrolled = Gtk.ScrolledWindow()
        scrolled.set_vexpand(True)
        self.append(scrolled)

        self.text_view = Gtk.TextView()
        self.text_view.set_editable(False)
        self.text_view.set_cursor_visible(False)
        self.text_view.set_monospace(True)
        self.text_view.set_wrap_mode(Gtk.WrapMode.WORD_CHAR)
        scrolled.set_child(self.text_view)

        self.buffer = self.text_view.get_buffer()
        self._line_count = 0

        self.tag_info = self.buffer.create_tag("info", foreground="#ffffff")
        self.tag_warn = self.buffer.create_tag("warn", foreground="#ff9800")
        self.tag_err = self.buffer.create_tag("err", foreground="#f44336")

        self._vadjustment = scrolled.get_vadjustment()

    def _is_scrolled_to_bottom(self) -> bool:
        adj = self._vadjustment
        return adj.get_value() >= adj.get_upper() - adj.get_page_size() - 20

    def append_line(self, text: str) -> None:
        was_at_bottom = self._is_scrolled_to_bottom()

        upper = text.upper()
        if "FATAL" in upper or "ERROR" in upper or "ERR" in upper:
            tag = self.tag_err
        elif "WARN" in upper:
            tag = self.tag_warn
        else:
            tag = self.tag_info

        timestamp = time.strftime("%H:%M:%S")
        line = f"[{timestamp}] {text}\n"

        end_iter = self.buffer.get_end_iter()
        self.buffer.insert_with_tags(end_iter, line, tag)
        self._line_count += 1

        if self._line_count > MAX_LINES:
            excess = self._line_count - MAX_LINES
            start_iter = self.buffer.get_start_iter()
            _found, cutoff_iter = self.buffer.get_iter_at_line(excess)
            self.buffer.delete(start_iter, cutoff_iter)
            self._line_count = MAX_LINES

        if was_at_bottom:
            self.text_view.scroll_to_iter(self.buffer.get_end_iter(), 0, False, 0, 0)

    def clear(self) -> None:
        self.buffer.set_text("")
        self._line_count = 0

    def copy_to_clipboard(self) -> None:
        text = self.buffer.get_text(
            self.buffer.get_start_iter(), self.buffer.get_end_iter(), True
        )
        self.get_clipboard().set(text)
