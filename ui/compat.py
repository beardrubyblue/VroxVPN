"""Замены Adw-виджетов, которых нет в libadwaita < 1.3..1.5 (например,
Ubuntu 22.04 ставит из своих репозиториев только 1.1.7) — без этого
приложение падает молча при сборке главного окна (исключение в do_activate
не печатается в терминал нагляднее, чем "процесс просто завершился").
Каждый класс повторяет ровно тот узкий API, который использует main_window.py."""
import gi

gi.require_version("Gtk", "4.0")
gi.require_version("Adw", "1")
from gi.repository import Gtk, Adw, GObject, GLib


class CompatBanner(Gtk.Revealer):
    """Замена Adw.Banner (появился в libadwaita 1.3)."""

    __gsignals__ = {
        "button-clicked": (GObject.SignalFlags.RUN_FIRST, None, ()),
    }

    def __init__(self):
        super().__init__()
        self.add_css_class("vrox-banner")
        self.set_transition_type(Gtk.RevealerTransitionType.SLIDE_DOWN)

        box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=8)
        box.set_margin_top(6)
        box.set_margin_bottom(6)
        box.set_margin_start(12)
        box.set_margin_end(12)

        self._label = Gtk.Label(label="")
        self._label.set_halign(Gtk.Align.START)
        self._label.set_hexpand(True)
        self._label.set_wrap(True)
        box.append(self._label)

        self._button = Gtk.Button()
        self._button.set_visible(False)
        self._button.connect("clicked", lambda _b: self.emit("button-clicked"))
        box.append(self._button)

        self.set_child(box)

    def set_title(self, text: str) -> None:
        self._label.set_text(text)

    def set_button_label(self, text: str) -> None:
        self._button.set_label(text or "")
        self._button.set_visible(bool(text))

    def set_revealed(self, revealed: bool) -> None:
        self.set_reveal_child(revealed)


class CompatSwitchRow(Adw.ActionRow):
    """Замена Adw.SwitchRow (появился в libadwaita 1.4) — ActionRow с
    Gtk.Switch в качестве suffix-виджета."""

    active = GObject.Property(type=bool, default=False)

    def __init__(self):
        super().__init__()
        self._switch = Gtk.Switch()
        self._switch.set_valign(Gtk.Align.CENTER)
        self.add_suffix(self._switch)
        self.set_activatable_widget(self._switch)
        self._switch.bind_property(
            "active", self, "active",
            GObject.BindingFlags.BIDIRECTIONAL | GObject.BindingFlags.SYNC_CREATE,
        )

    def set_active(self, value: bool) -> None:
        self.set_property("active", value)

    def get_active(self) -> bool:
        return self.get_property("active")


SUGGESTED = "suggested"
DESTRUCTIVE = "destructive"


SHEET_TRANSITION_MS = 250


class BottomSheet:
    """Выезжающий снизу 'sheet' с затемнением фона — повторяет адаптивную
    презентацию Adw.Dialog на узких окнах (появилась в libadwaita 1.5,
    которой нет в 1.1 на Ubuntu 22.04). Рисуется внутри Gtk.Overlay
    родительского окна, а не отдельным top-level окном — поэтому может
    выезжать именно из нижнего края окна приложения, а не появляться
    обычным окном ОС."""

    def __init__(self, overlay: Gtk.Overlay, content: Gtk.Widget):
        self._overlay = overlay

        self._scrim = Gtk.Box()
        self._scrim.add_css_class("vrox-sheet-scrim")
        self._scrim.set_hexpand(True)
        self._scrim.set_vexpand(True)
        click = Gtk.GestureClick()
        click.connect("released", lambda *_a: self.close())
        self._scrim.add_controller(click)

        sheet_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL)
        sheet_box.add_css_class("vrox-sheet")
        sheet_box.append(content)

        self._revealer = Gtk.Revealer()
        self._revealer.set_transition_type(Gtk.RevealerTransitionType.SLIDE_UP)
        self._revealer.set_transition_duration(SHEET_TRANSITION_MS)
        self._revealer.set_valign(Gtk.Align.END)
        self._revealer.set_child(sheet_box)

        overlay.add_overlay(self._scrim)
        overlay.add_overlay(self._revealer)

    def present(self) -> None:
        GLib.idle_add(self._reveal)

    def _reveal(self) -> bool:
        self._scrim.add_css_class("visible")
        self._revealer.set_reveal_child(True)
        return False

    def close(self) -> None:
        self._scrim.remove_css_class("visible")
        self._revealer.set_reveal_child(False)
        GLib.timeout_add(SHEET_TRANSITION_MS, self._cleanup)

    def _cleanup(self) -> bool:
        self._overlay.remove_overlay(self._scrim)
        self._overlay.remove_overlay(self._revealer)
        return False


class CompatAlertDialog:
    """Замена Adw.AlertDialog/Adw.ResponseAppearance (появились в
    libadwaita 1.5) — выезжающий снизу sheet внутри окна (BottomSheet),
    а не отдельное окно ОС: раньше это было Gtk.Window, которое не
    ограничено шириной нашего окна и раздувалось в ширину на длинном
    тексте (например, changelog обновления). Тот же узкий API:
    heading/body в конструкторе, add_response, set_response_appearance,
    set_default_response, set_extra_child, сигнал "response" с
    (dialog, response_id: str), present(parent), где parent — окно с
    атрибутом _overlay (см. MainWindow)."""

    def __init__(self, heading: str = "", body: str = ""):
        self._buttons = {}
        self._default_response = None
        self._responded = False
        self._response_callbacks = []
        self._sheet = None

        self._box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=12)
        self._box.set_margin_top(20)
        self._box.set_margin_bottom(20)
        self._box.set_margin_start(20)
        self._box.set_margin_end(20)

        if heading:
            heading_label = Gtk.Label(label=heading)
            heading_label.add_css_class("title-2")
            heading_label.set_wrap(True)
            heading_label.set_halign(Gtk.Align.START)
            self._box.append(heading_label)

        if body:
            body_label = Gtk.Label(label=body)
            body_label.set_wrap(True)
            body_label.set_halign(Gtk.Align.START)
            self._box.append(body_label)

        self._extra_slot = Gtk.Box(orientation=Gtk.Orientation.VERTICAL)
        self._box.append(self._extra_slot)

        self._button_box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=8)
        self._button_box.set_halign(Gtk.Align.END)
        self._button_box.set_margin_top(8)
        self._box.append(self._button_box)

    def set_extra_child(self, widget: Gtk.Widget) -> None:
        self._extra_slot.append(widget)

    def add_response(self, response_id: str, label: str) -> None:
        button = Gtk.Button(label=label)
        button.connect("clicked", lambda _b, rid=response_id: self._respond(rid))
        self._button_box.append(button)
        self._buttons[response_id] = button

    def set_response_appearance(self, response_id: str, appearance: str) -> None:
        button = self._buttons.get(response_id)
        if not button:
            return
        if appearance == SUGGESTED:
            button.add_css_class("suggested-action")
        elif appearance == DESTRUCTIVE:
            button.add_css_class("destructive-action")

    def set_default_response(self, response_id: str) -> None:
        self._default_response = response_id

    def connect(self, signal_name: str, callback) -> None:
        if signal_name == "response":
            self._response_callbacks.append(callback)

    def _respond(self, response_id: str) -> None:
        if self._responded:
            return
        self._responded = True
        for callback in self._response_callbacks:
            callback(self, response_id)
        if self._sheet:
            self._sheet.close()

    def present(self, parent) -> None:
        self._sheet = BottomSheet(parent._overlay, self._box)
        self._sheet.present()
