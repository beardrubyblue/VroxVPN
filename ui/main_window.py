"""Главное окно vrox.vpn."""
import os
import sys
import threading
import time

import gi

gi.require_version("Gtk", "4.0")
gi.require_version("Adw", "1")
from gi.repository import Gtk, Adw, GLib, Gio

from core import settings, geoip
from core.subscription import fetch_subscription
from core.tun_manager import TunManager
from core.installer import is_installed, download_hysteria2
from core.ping import ping_all_servers
from core.kill_switch import KillSwitch
from core.stats import TrafficStats
from core.updater import Updater, AppUpdater
from ui.server_row import ServerRow
from ui.compat import CompatBanner, CompatSwitchRow, CompatAlertDialog, SUGGESTED, BottomSheet
from ui.stats_bar import StatsBar
from ui.log_panel import LogPanel

APP_VERSION = "2.2.15"


def _format_userinfo(userinfo: dict) -> str:
    """Строка вида " · 2.1/50 ГБ · до 30.07.2026" из заголовка
    Subscription-Userinfo (upload/download/total/expire). total/expire
    равные 0 у 3x-ui означают "без лимита"/"бессрочно" — в этом случае
    соответствующую часть не показываем."""
    parts = []

    total = userinfo.get("total", 0)
    if total:
        used_gb = (userinfo.get("upload", 0) + userinfo.get("download", 0)) / (1024 ** 3)
        total_gb = total / (1024 ** 3)
        parts.append(f"{used_gb:.1f}/{total_gb:.1f} ГБ")

    expire = userinfo.get("expire", 0)
    if expire:
        parts.append(f"до {time.strftime('%d.%m.%Y', time.localtime(expire))}")

    return " · " + " · ".join(parts) if parts else ""


class MainWindow(Adw.ApplicationWindow):
    def __init__(self, app):
        super().__init__(application=app)
        self.set_title("vrox.vpn")
        self.set_default_size(420, 680)

        self.tun_manager = TunManager()
        self.tun_manager.on_connected = self._on_tun_connected
        self.tun_manager.on_disconnected = self._on_tun_disconnected
        self.tun_manager.on_error = self._on_tun_error
        self.tun_manager.on_log = self._on_tun_log
        self.tun_manager.on_reconnecting = self._on_tun_reconnecting

        self.kill_switch = KillSwitch()
        # Kill Switch и DNS защита временно скрыты из UI — не работают стабильно
        self.tun_manager.dns_protection_enabled = False

        self.stats = TrafficStats()
        self.stats.on_update = self._on_stats_update

        self.updater = Updater()
        self.app_updater = AppUpdater()

        self._servers = []
        self._selected_server = None
        self._state = "idle"  # idle | connecting | connected | disconnecting
        self.on_servers_updated = None

        self._setup_actions()
        self._build_ui()
        self._load_initial_state()

    # ------------------------------------------------------------ actions

    def _setup_actions(self):
        action = Gio.SimpleAction.new("subscription-settings", None)
        action.connect("activate", lambda *_a: self._on_settings_clicked(None))
        self.add_action(action)

        action = Gio.SimpleAction.new("check-updates", None)
        action.connect("activate", lambda *_a: self._manual_check_updates())
        self.add_action(action)

        action = Gio.SimpleAction.new("check-app-updates", None)
        action.connect("activate", lambda *_a: self._manual_check_app_updates())
        self.add_action(action)

        action = Gio.SimpleAction.new("about", None)
        action.connect("activate", lambda *_a: self._show_about_dialog())
        self.add_action(action)

        action = Gio.SimpleAction.new("quit", None)
        action.connect("activate", lambda *_a: self._on_quit_clicked())
        self.add_action(action)

    # ---------------------------------------------------------------- UI

    def _build_ui(self):
        root = Gtk.Box(orientation=Gtk.Orientation.VERTICAL)

        # overlay нужен, чтобы "О программе" могло выезжать снизу поверх
        # содержимого окна (см. _show_about_dialog) — как раньше делал
        # Adw.AboutDialog на узких окнах через адаптивную bottom-sheet
        # презентацию (libadwaita 1.5+, недоступна на Ubuntu 22.04)
        self._overlay = Gtk.Overlay()
        self._overlay.set_child(root)
        self.set_content(self._overlay)

        header = Adw.HeaderBar()

        refresh_btn = Gtk.Button()
        refresh_btn.set_icon_name("view-refresh-symbolic")
        refresh_btn.set_tooltip_text("Обновить подписку")
        refresh_btn.connect("clicked", self._on_refresh_clicked)
        header.pack_start(refresh_btn)

        self.window_title = Adw.WindowTitle(title="vrox.vpn", subtitle="Не подключено")
        header.set_title_widget(self.window_title)

        menu = Gio.Menu()
        menu.append("Настройки подписки", "win.subscription-settings")
        menu.append("Проверить обновления hysteria2", "win.check-updates")
        menu.append("Проверить обновления приложения", "win.check-app-updates")
        about_section = Gio.Menu()
        about_section.append("О программе", "win.about")
        menu.append_section(None, about_section)

        quit_section = Gio.Menu()
        quit_section.append("Выйти полностью", "win.quit")
        menu.append_section(None, quit_section)

        menu_button = Gtk.MenuButton()
        menu_button.set_icon_name("open-menu-symbolic")
        menu_button.set_popover(Gtk.PopoverMenu.new_from_model(menu))
        header.pack_end(menu_button)

        root.append(header)

        self.progress_bar = Gtk.ProgressBar()
        self.progress_bar.set_visible(False)
        root.append(self.progress_bar)

        self.banner = CompatBanner()
        self.banner.set_revealed(False)
        self.banner.connect("button-clicked", self._on_banner_button_clicked)
        self._banner_click_handler = None
        self._banner_timeout_id = None
        root.append(self.banner)

        # отдельный баннер для обновлений самого приложения (не hysteria2)
        self.app_update_banner = CompatBanner()
        self.app_update_banner.set_revealed(False)
        self.app_update_banner.connect("button-clicked", self._on_app_banner_button_clicked)
        self._app_banner_click_handler = None
        self._app_banner_timeout_id = None
        root.append(self.app_update_banner)

        self.stack = Adw.ViewStack()
        self.stack.set_vexpand(True)
        root.append(self.stack)

        home_page = self._build_home_page()
        home_page_ref = self.stack.add_titled(home_page, "home", "Главная")
        home_page_ref.set_icon_name("go-home-symbolic")

        self.log_panel = LogPanel()
        logs_page_ref = self.stack.add_titled(self.log_panel, "logs", "Логи")
        logs_page_ref.set_icon_name("text-x-generic-symbolic")

        settings_page = self._build_settings_page()
        settings_page_ref = self.stack.add_titled(settings_page, "settings", "Настройки")
        settings_page_ref.set_icon_name("preferences-system-symbolic")

        view_switcher = Adw.ViewSwitcherBar()
        view_switcher.set_stack(self.stack)
        view_switcher.set_reveal(True)
        root.append(view_switcher)

        self._apply_css()
        self._set_status("idle", "Не подключено")
        self._update_connect_button()

    def _build_home_page(self) -> Gtk.Widget:
        page = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=18)
        page.set_margin_top(18)
        page.set_margin_bottom(12)
        page.set_margin_start(18)
        page.set_margin_end(18)
        page.set_vexpand(True)

        # --- карточка статуса ---
        status_group = Adw.PreferencesGroup()

        self.status_row = Adw.ActionRow()
        self.status_row.set_margin_top(8)
        self.status_row.set_margin_bottom(8)

        self.status_icon = Gtk.Image.new_from_icon_name("network-vpn-symbolic")
        self.status_icon.set_pixel_size(48)
        self.status_icon.add_css_class("dim-label")
        self.status_row.add_prefix(self.status_icon)

        self.status_badge = Gtk.Label(label="ВЫКЛ")
        self.status_badge.add_css_class("status-badge")
        self.status_badge.add_css_class("disconnected")
        self.status_badge.set_valign(Gtk.Align.CENTER)
        self.status_row.add_suffix(self.status_badge)

        status_group.add(self.status_row)
        page.append(status_group)

        # --- статистика трафика ---
        self.stats_bar = StatsBar()
        page.append(self.stats_bar)

        # --- список серверов ---
        self.servers_group = Adw.PreferencesGroup()
        self.servers_group.set_title("Серверы")
        self.servers_group.set_description("Нет серверов")
        self.servers_group.set_vexpand(True)

        self.ping_button = Gtk.Button()
        self.ping_button.set_icon_name("network-wired-symbolic")
        self.ping_button.set_tooltip_text("Проверить пинг всех серверов")
        self.ping_button.add_css_class("flat")
        self.ping_button.set_valign(Gtk.Align.CENTER)
        self.ping_button.connect("clicked", self._on_ping_button_clicked)
        self.servers_group.set_header_suffix(self.ping_button)

        scrolled = Gtk.ScrolledWindow()
        scrolled.set_vexpand(True)
        scrolled.set_has_frame(False)

        self.list_box = Gtk.ListBox()
        self.list_box.add_css_class("boxed-list")
        self.list_box.set_selection_mode(Gtk.SelectionMode.SINGLE)
        self.list_box.connect("row-selected", self._on_row_selected)

        placeholder = Adw.StatusPage()
        placeholder.set_icon_name("network-wireless-symbolic")
        placeholder.set_title("Серверы не загружены")
        placeholder.set_description("Нажмите ⟳")
        self.list_box.set_placeholder(placeholder)

        scrolled.set_child(self.list_box)
        self.servers_group.add(scrolled)
        page.append(self.servers_group)

        # --- защита (Kill Switch / DNS) — временно скрыто, нестабильно ---
        protection_group = Adw.PreferencesGroup()
        protection_group.set_title("Защита")
        protection_group.set_visible(False)

        self.kill_switch_toggle = CompatSwitchRow()
        self.kill_switch_toggle.set_title("Kill Switch")
        self.kill_switch_toggle.set_subtitle("Блокировать трафик без VPN")
        self.kill_switch_toggle.set_icon_name("network-offline-symbolic")
        self.kill_switch_toggle.set_active(False)
        self.kill_switch_toggle.connect("notify::active", self._on_kill_switch_toggled)
        protection_group.add(self.kill_switch_toggle)

        self.dns_toggle = CompatSwitchRow()
        self.dns_toggle.set_title("DNS защита")
        self.dns_toggle.set_subtitle("Предотвращение DNS утечек")
        self.dns_toggle.set_icon_name("system-lock-screen-symbolic")
        self.dns_toggle.set_active(False)
        self.dns_toggle.connect("notify::active", self._on_dns_toggled)
        protection_group.add(self.dns_toggle)

        page.append(protection_group)

        # --- кнопка подключения ---
        self.connect_button = Gtk.Button()
        self.connect_button.add_css_class("pill")
        self.connect_button.add_css_class("connect-button")
        self.connect_button.set_size_request(-1, 52)
        self.connect_button.set_margin_top(8)
        self.connect_button.connect("clicked", self._on_connect_clicked)
        page.append(self.connect_button)

        return page

    def _build_settings_page(self) -> Gtk.Widget:
        page = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=18)
        page.set_margin_top(18)
        page.set_margin_bottom(12)
        page.set_margin_start(18)
        page.set_margin_end(18)

        routing_group = Adw.PreferencesGroup()
        routing_group.set_title("Маршрутизация")

        self.ru_bypass_toggle = CompatSwitchRow()
        self.ru_bypass_toggle.set_title("Российские сервисы напрямую")
        self.ru_bypass_toggle.set_subtitle("Не пускать IP-адреса российских сервисов через VPN")
        self.ru_bypass_toggle.set_icon_name("network-transmit-receive-symbolic")
        self.ru_bypass_toggle.set_active(settings.get("ru_bypass_enabled", False))
        self.ru_bypass_toggle.connect("notify::active", self._on_ru_bypass_toggled)
        routing_group.add(self.ru_bypass_toggle)

        self.geoip_update_row = Adw.ActionRow()
        self.geoip_update_row.set_title("База IP-адресов России")
        self.geoip_update_row.set_subtitle(f"Обновлено: {geoip.last_updated()} · {geoip.current_size_kb():.0f} КБ")
        self.geoip_update_row.set_icon_name("view-refresh-symbolic")

        self.geoip_update_button = Gtk.Button(label="Обновить")
        self.geoip_update_button.set_valign(Gtk.Align.CENTER)
        self.geoip_update_button.connect("clicked", self._on_geoip_update_clicked)
        self.geoip_update_row.add_suffix(self.geoip_update_button)
        routing_group.add(self.geoip_update_row)

        page.append(routing_group)

        return page

    def _apply_css(self):
        css = b"""
        .status-badge {
            border-radius: 6px;
            padding: 2px 8px;
            font-size: 11px;
            font-weight: bold;
        }
        .status-badge.connected {
            background-color: alpha(@success_color, 0.15);
            color: @success_color;
        }
        .status-badge.disconnected {
            background-color: alpha(@window_fg_color, 0.08);
            color: alpha(@window_fg_color, 0.4);
        }
        .ping-warn {
            color: #e5a50a;
        }
        .connect-button label {
            font-size: 15px;
            font-weight: 600;
        }
        .vrox-sheet-scrim {
            background-color: rgba(0, 0, 0, 0);
            transition: background-color 250ms ease-out;
        }
        .vrox-sheet-scrim.visible {
            background-color: rgba(0, 0, 0, 0.45);
        }
        .vrox-sheet {
            background-color: @window_bg_color;
            border-top-left-radius: 14px;
            border-top-right-radius: 14px;
            box-shadow: 0 -2px 12px rgba(0, 0, 0, 0.25);
        }
        """
        provider = Gtk.CssProvider()
        provider.load_from_data(css)
        Gtk.StyleContext.add_provider_for_display(
            self.get_display(), provider, Gtk.STYLE_PROVIDER_PRIORITY_APPLICATION
        )

    # ------------------------------------------------------------ helpers

    def _set_status(self, kind: str, title: str, subtitle: str = ""):
        self.status_row.set_title(title)
        self.status_row.set_subtitle(subtitle)
        self.window_title.set_subtitle(title)

        is_connected = kind == "connected"

        self.status_icon.remove_css_class("success")
        self.status_icon.remove_css_class("dim-label")
        self.status_icon.add_css_class("success" if is_connected else "dim-label")

        self.status_badge.set_label("ВКЛ" if is_connected else "ВЫКЛ")
        self.status_badge.remove_css_class("connected")
        self.status_badge.remove_css_class("disconnected")
        self.status_badge.add_css_class("connected" if is_connected else "disconnected")

    def _update_connect_button(self):
        btn = self.connect_button
        btn.set_sensitive(True)

        for cls in ("suggested-action", "destructive-action"):
            btn.remove_css_class(cls)

        content_box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=8)
        content_box.set_halign(Gtk.Align.CENTER)

        if self._state == "idle":
            btn.add_css_class("suggested-action")
            icon = Gtk.Image.new_from_icon_name("network-vpn-symbolic")
            content_box.append(icon)
            content_box.append(Gtk.Label(label="Подключиться"))
        elif self._state == "connecting":
            btn.set_sensitive(False)
            spinner = Gtk.Spinner()
            spinner.start()
            content_box.append(spinner)
            content_box.append(Gtk.Label(label="Подключение…"))
        elif self._state == "connected":
            btn.add_css_class("destructive-action")
            icon = Gtk.Image.new_from_icon_name("network-vpn-symbolic")
            content_box.append(icon)
            content_box.append(Gtk.Label(label="Отключиться"))
        elif self._state == "disconnecting":
            btn.set_sensitive(False)
            spinner = Gtk.Spinner()
            spinner.start()
            content_box.append(spinner)
            content_box.append(Gtk.Label(label="Отключение…"))

        btn.set_child(content_box)

    def _show_banner(
        self,
        text: str,
        button_label: str = "",
        on_click=None,
        warning: bool = False,
        persistent: bool = False,
    ):
        self.banner.set_title(text)
        self.banner.set_button_label(button_label)
        self._banner_click_handler = on_click
        self.banner.set_revealed(True)
        if warning:
            self.banner.add_css_class("error")
        else:
            self.banner.remove_css_class("error")

        if self._banner_timeout_id:
            GLib.source_remove(self._banner_timeout_id)
            self._banner_timeout_id = None

        # ошибки не прячем автоматически — пользователь должен успеть
        # прочитать, что именно пошло не так (например, сеть недоступна)
        if not persistent and not warning:
            self._banner_timeout_id = GLib.timeout_add_seconds(5, self._auto_hide_banner)
        return False

    def _auto_hide_banner(self):
        self.banner.set_revealed(False)
        self._banner_timeout_id = None
        return False

    def _on_banner_button_clicked(self, _banner):
        if self._banner_click_handler:
            self._banner_click_handler()

    def _show_app_banner(self, text: str, button_label: str = "", on_click=None, persistent: bool = False):
        self.app_update_banner.set_title(text)
        self.app_update_banner.set_button_label(button_label)
        self._app_banner_click_handler = on_click
        self.app_update_banner.set_revealed(True)

        if self._app_banner_timeout_id:
            GLib.source_remove(self._app_banner_timeout_id)
            self._app_banner_timeout_id = None

        if not persistent:
            self._app_banner_timeout_id = GLib.timeout_add_seconds(5, self._auto_hide_app_banner)
        return False

    def _auto_hide_app_banner(self):
        self.app_update_banner.set_revealed(False)
        self._app_banner_timeout_id = None
        return False

    def _on_app_banner_button_clicked(self, _banner):
        if self._app_banner_click_handler:
            self._app_banner_click_handler()

    def _show_about_dialog(self):
        # Выезжающий снизу sheet вместо отдельного окна — раньше так же
        # выглядел Adw.AboutDialog на узких окнах (адаптивная bottom-sheet
        # презентация, появилась в libadwaita 1.5, недоступна на Ubuntu
        # 22.04 / libadwaita 1.1, поэтому сам класс заменён на Gtk-виджеты,
        # но визуальное поведение "выезжает снизу" сохранено через BottomSheet)
        box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=8)
        box.set_margin_top(24)
        box.set_margin_bottom(24)
        box.set_margin_start(24)
        box.set_margin_end(24)

        close_row = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL)
        close_row.set_halign(Gtk.Align.END)
        close_btn = Gtk.Button()
        close_btn.set_icon_name("window-close-symbolic")
        close_btn.add_css_class("flat")
        close_btn.add_css_class("circular")
        close_btn.connect("clicked", lambda _b: sheet.close())
        close_row.append(close_btn)
        box.append(close_row)

        icon = Gtk.Image.new_from_icon_name("com.vroxory.vpn")
        icon.set_pixel_size(64)
        icon.set_margin_bottom(8)
        box.append(icon)

        name_label = Gtk.Label(label="vrox.vpn")
        name_label.add_css_class("title-1")
        box.append(name_label)

        version_label = Gtk.Label(label=f"Версия {APP_VERSION}")
        version_label.add_css_class("dim-label")
        box.append(version_label)

        comments_label = Gtk.Label(label="Hysteria2 VPN клиент с TUN режимом для Linux")
        comments_label.set_wrap(True)
        comments_label.set_justify(Gtk.Justification.CENTER)
        comments_label.set_margin_top(12)
        box.append(comments_label)

        website_btn = Gtk.LinkButton(uri="https://net.vroxory.com", label="net.vroxory.com")
        website_btn.set_halign(Gtk.Align.CENTER)
        box.append(website_btn)

        license_label = Gtk.Label(label="Vroxory · Лицензия MIT")
        license_label.add_css_class("dim-label")
        license_label.add_css_class("caption")
        license_label.set_margin_top(12)
        box.append(license_label)

        sheet = BottomSheet(self._overlay, box)
        sheet.present()

    def _on_quit_clicked(self):
        app = self.get_application()
        if hasattr(app, "request_full_quit"):
            app.request_full_quit()
        else:
            app.quit()

    # ----------------------------------------------------------- loading

    def _load_initial_state(self):
        url = settings.get("subscription_url", "")
        if url:
            self._fetch_subscription(url)
        self._check_for_updates()
        self._check_app_update()

    def _check_for_updates(self):
        def worker():
            try:
                result = self.updater.check_update()
            except Exception:
                return
            if result["update_available"] and result["latest"]:
                GLib.idle_add(self._show_update_banner, result["latest"])

        threading.Thread(target=worker, daemon=True).start()

    def _manual_check_updates(self):
        self._show_banner("Проверка обновлений…")

        def worker():
            try:
                result = self.updater.check_update()
            except Exception as exc:
                GLib.idle_add(self._show_banner, f"Ошибка проверки обновлений: {exc}")
                return
            if result["update_available"] and result["latest"]:
                GLib.idle_add(self._show_update_banner, result["latest"])
            else:
                GLib.idle_add(self._show_banner, "У вас установлена последняя версия hysteria2")

        threading.Thread(target=worker, daemon=True).start()

    def _show_update_banner(self, latest_version: str):
        self._show_banner(
            f"Доступно обновление hysteria2: {latest_version}",
            button_label="Обновить",
            on_click=self._start_update,
            persistent=True,
        )
        return False

    def _start_update(self):
        self.progress_bar.set_visible(True)
        self.progress_bar.set_fraction(0)
        self._show_banner("Обновление hysteria2…")

        def progress_callback(downloaded, total):
            if total:
                GLib.idle_add(self.progress_bar.set_fraction, downloaded / total)

        def worker():
            success = self.updater.update(progress_callback)
            GLib.idle_add(self._on_update_finished, success)

        threading.Thread(target=worker, daemon=True).start()

    def _on_update_finished(self, success: bool):
        self.progress_bar.set_visible(False)
        if success:
            self._show_banner("✓ hysteria2 обновлён")
        else:
            self._show_banner("Ошибка обновления hysteria2", warning=True)
        return False

    # ----------------------------------------------- обновление приложения

    def _check_app_update(self):
        def worker():
            result = self.app_updater.check_update()
            if result and result["update_available"]:
                GLib.idle_add(self._show_app_update_banner, result)

        threading.Thread(target=worker, daemon=True).start()

    def _manual_check_app_updates(self):
        self._show_app_banner("Проверка обновлений приложения…")

        def worker():
            result = self.app_updater.check_update()
            if result and result["update_available"]:
                GLib.idle_add(self._show_app_update_banner, result)
            elif result:
                GLib.idle_add(self._show_app_banner, "У вас установлена последняя версия приложения")
            else:
                GLib.idle_add(self._show_app_banner, "Не удалось проверить обновления приложения")

        threading.Thread(target=worker, daemon=True).start()

    def _show_app_update_banner(self, result: dict):
        self._show_app_banner(
            f"Доступно обновление vrox.vpn {result['latest']}",
            button_label="Обновить",
            on_click=lambda: self._prompt_app_update(result),
            persistent=True,
        )
        return False

    def _prompt_app_update(self, result: dict):
        dialog = CompatAlertDialog(
            heading=f"Обновление до версии {result['latest']}",
            body=f"{result['changelog']}\n\nПриложение будет перезапущено после обновления.",
        )
        dialog.add_response("cancel", "Отмена")
        dialog.add_response("install", "Установить")
        dialog.set_response_appearance("install", SUGGESTED)
        dialog.set_default_response("install")

        def on_response(_dialog, response):
            if response == "install":
                self._start_app_update(result)

        dialog.connect("response", on_response)
        dialog.present(self)

    def _start_app_update(self, result: dict):
        self.app_update_banner.set_revealed(False)
        self.progress_bar.set_visible(True)
        self.progress_bar.set_fraction(0)

        def progress_callback(downloaded, total):
            if total:
                GLib.idle_add(self.progress_bar.set_fraction, downloaded / total)

        def worker():
            success = self.app_updater.download_and_install(
                result["download_url"],
                result.get("sha256", ""),
                progress_callback,
            )
            GLib.idle_add(self._on_app_update_finished, success)

        threading.Thread(target=worker, daemon=True).start()

    def _on_app_update_finished(self, success: bool):
        self.progress_bar.set_visible(False)
        if success:
            self._prompt_restart()
        else:
            self._show_app_banner("Ошибка установки обновления приложения")
        return False

    def _prompt_restart(self):
        dialog = CompatAlertDialog(
            heading="Обновление установлено",
            body="Обновление установлено. Перезапустите приложение.",
        )
        dialog.add_response("later", "Позже")
        dialog.add_response("restart", "Перезапустить")
        dialog.set_response_appearance("restart", SUGGESTED)
        dialog.set_default_response("restart")

        def on_response(_dialog, response):
            if response == "restart":
                # os.execv() заменяет только образ ТЕКУЩЕГО процесса — отдельный
                # GTK3-процесс трея (core/tray_process.py), запущенный через
                # subprocess.Popen, продолжает жить как есть и теряет связь с
                # новым (перезапущенным) процессом. tray.start() после
                # перезапуска поднимает ещё один — получаются два процесса
                # трея, один из которых не закрыть иначе как из системного
                # монитора. Поэтому старый процесс трея нужно остановить
                # явно, ДО execv.
                app = self.get_application()
                if hasattr(app, "tray"):
                    app.tray.stop()
                os.execv(sys.executable, [sys.executable] + sys.argv)

        dialog.connect("response", on_response)
        dialog.present(self)

    def _populate_servers(self, servers: list, userinfo: dict = None):
        self._servers = servers

        while True:
            row = self.list_box.get_row_at_index(0)
            if row is None:
                break
            self.list_box.remove(row)

        for server in servers:
            row = ServerRow(server)
            self.list_box.append(row)

        count = len(servers)
        base = f"{count} доступно" if count else "Нет серверов"
        suffix = _format_userinfo(userinfo or {})
        self.servers_group.set_description(base + suffix)

        last_name = settings.get("last_selected_server", "")
        index_to_select = 0
        for i, server in enumerate(servers):
            if server["name"] == last_name:
                index_to_select = i
                break

        if servers:
            row = self.list_box.get_row_at_index(index_to_select)
            self.list_box.select_row(row)

        if self.on_servers_updated:
            self.on_servers_updated(servers)

    def select_server_by_name(self, name: str) -> bool:
        """Выбирает сервер по имени — используется треем для выбора без
        открытия главного окна."""
        for i, server in enumerate(self._servers):
            if server["name"] != name:
                continue
            row = self.list_box.get_row_at_index(i)
            if row is not None:
                self.list_box.select_row(row)
            else:
                self._selected_server = server
                settings.set("last_selected_server", name)
            return True
        return False

    def _fetch_subscription(self, url: str):
        self.progress_bar.set_visible(True)
        self.progress_bar.pulse()

        def worker():
            try:
                servers, userinfo = fetch_subscription(url)
            except Exception as exc:
                GLib.idle_add(self._on_fetch_error, str(exc))
                return
            GLib.idle_add(self._on_fetch_success, servers, userinfo)

        threading.Thread(target=worker, daemon=True).start()

    def _on_fetch_success(self, servers: list, userinfo: dict):
        self.progress_bar.set_visible(False)
        self._populate_servers(servers, userinfo)
        if servers:
            self._show_banner(f"Загружено серверов: {len(servers)}")
            self._ping_servers()
        else:
            self._show_banner("Подписка пуста или не распознана")
        return False

    def _on_ping_button_clicked(self, _button):
        if not self._servers:
            self._show_banner("Нет серверов для проверки")
            return
        self._ping_servers()

    def _ping_servers(self):
        servers = list(self._servers)
        self.ping_button.set_sensitive(False)

        pending = len(servers)
        lock = threading.Lock()

        def on_ping_result(name: str, latency_ms):
            nonlocal pending
            GLib.idle_add(self._apply_ping_result, name, latency_ms)
            with lock:
                pending -= 1
                if pending <= 0:
                    GLib.idle_add(self._on_ping_finished)

        def worker():
            ping_all_servers(servers, on_ping_result)

        threading.Thread(target=worker, daemon=True).start()

    def _on_ping_finished(self):
        self.ping_button.set_sensitive(True)
        return False

    def _apply_ping_result(self, name: str, latency_ms):
        for i in range(len(self._servers)):
            if self._servers[i]["name"] != name:
                continue
            row = self.list_box.get_row_at_index(i)
            if row is not None:
                row.set_ping(latency_ms)
            break
        return False

    def _on_fetch_error(self, message: str):
        self.progress_bar.set_visible(False)
        self._show_banner(f"Ошибка загрузки подписки: {message}", warning=True)
        return False

    # ------------------------------------------------------------ events

    def _on_refresh_clicked(self, _button):
        url = settings.get("subscription_url", "")
        if not url:
            self._show_banner("Сначала укажите URL подписки в настройках")
            return
        self._fetch_subscription(url)

    def _on_settings_clicked(self, _button):
        dialog = CompatAlertDialog(
            heading="URL подписки",
            body="Введите ссылку на подписку hysteria2",
        )
        entry = Gtk.Entry()
        entry.set_text(settings.get("subscription_url", ""))
        dialog.set_extra_child(entry)
        dialog.add_response("cancel", "Отмена")
        dialog.add_response("save", "Сохранить")
        dialog.set_response_appearance("save", SUGGESTED)
        dialog.set_default_response("save")

        def on_response(_dialog, response):
            if response == "save":
                url = entry.get_text().strip()
                settings.set("subscription_url", url)
                if url:
                    self._fetch_subscription(url)

        dialog.connect("response", on_response)
        dialog.present(self)

    def _on_kill_switch_toggled(self, switch, _pspec):
        enabled = switch.get_active()
        settings.set("kill_switch_enabled", enabled)
        if self._state == "connected":
            if enabled:
                self._enable_kill_switch()
            else:
                self._disable_kill_switch()

    def _enable_kill_switch(self):
        server_ip = self._selected_server["host"] if self._selected_server else ""

        def worker():
            self.kill_switch.enable(vpn_server_ip=server_ip)

        threading.Thread(target=worker, daemon=True).start()

    def _disable_kill_switch(self):
        def worker():
            self.kill_switch.disable()

        threading.Thread(target=worker, daemon=True).start()

    def _on_dns_toggled(self, switch, _pspec):
        enabled = switch.get_active()
        settings.set("dns_protection_enabled", enabled)
        self.tun_manager.dns_protection_enabled = enabled

        if self._state == "connected":
            def worker():
                if enabled:
                    self.tun_manager.dns_manager.enable()
                else:
                    self.tun_manager.dns_manager.disable()

            threading.Thread(target=worker, daemon=True).start()

    def _on_ru_bypass_toggled(self, switch, _pspec):
        enabled = switch.get_active()
        settings.set("ru_bypass_enabled", enabled)
        if self._state == "connected":
            self._show_banner("Изменения применятся при следующем подключении")

    def _on_geoip_update_clicked(self, _button):
        self.geoip_update_button.set_sensitive(False)

        def worker():
            try:
                result = geoip.update_ru_cidrs()
            except Exception as exc:
                GLib.idle_add(self._on_geoip_update_error, str(exc))
                return
            GLib.idle_add(self._on_geoip_update_success, result)

        threading.Thread(target=worker, daemon=True).start()

    def _on_geoip_update_success(self, result: dict):
        self.geoip_update_button.set_sensitive(True)
        size_kb = result["bytes"] / 1024
        self.geoip_update_row.set_subtitle(f"Обновлено: {geoip.last_updated()} · {size_kb:.0f} КБ")
        self._show_banner(f"База обновлена: {result['count']} диапазонов, {size_kb:.0f} КБ")
        return False

    def _on_geoip_update_error(self, message: str):
        self.geoip_update_button.set_sensitive(True)
        self._show_banner(f"Не удалось обновить базу: {message}", warning=True)
        return False

    def _on_row_selected(self, _list_box, row):
        if row is None:
            self._selected_server = None
            return
        index = row.get_index()
        if 0 <= index < len(self._servers):
            self._selected_server = self._servers[index]
            settings.set("last_selected_server", self._selected_server["name"])

    def _on_connect_clicked(self, _button):
        if self._state == "connected":
            self._start_disconnect()
            return

        if self._state in ("connecting", "disconnecting"):
            return

        if not self._selected_server:
            self._show_banner("Выберите сервер")
            return

        if not is_installed():
            self._prompt_install()
            return

        self._start_connect()

    def _prompt_install(self):
        dialog = CompatAlertDialog(
            heading="hysteria2 не установлен",
            body="Для подключения нужен бинарник hysteria2. Скачать сейчас?",
        )
        dialog.add_response("cancel", "Отмена")
        dialog.add_response("install", "Установить")
        dialog.set_response_appearance("install", SUGGESTED)
        dialog.set_default_response("install")

        def on_response(_dialog, response):
            if response == "install":
                self._install_hysteria2()

        dialog.connect("response", on_response)
        dialog.present(self)

    def _install_hysteria2(self):
        self.progress_bar.set_visible(True)
        self.progress_bar.set_fraction(0)

        def progress_callback(downloaded, total):
            if total:
                GLib.idle_add(self.progress_bar.set_fraction, downloaded / total)

        def worker():
            try:
                download_hysteria2(progress_callback)
            except Exception as exc:
                GLib.idle_add(self._on_install_error, str(exc))
                return
            GLib.idle_add(self._on_install_success)

        threading.Thread(target=worker, daemon=True).start()

    def _on_install_success(self):
        self.progress_bar.set_visible(False)
        self._show_banner("hysteria2 установлен")
        return False

    def _on_install_error(self, message: str):
        self.progress_bar.set_visible(False)
        self._show_banner(f"Ошибка установки: {message}", warning=True)
        return False

    # --------------------------------------------------------- connection

    def _start_connect(self):
        self._state = "connecting"
        self._set_status("connecting", "Подключение…")
        self._update_connect_button()

        if self._selected_server.get("insecure"):
            self._show_banner(
                "⚠ Этот сервер отключает проверку TLS-сертификата (insecure)",
                warning=True,
            )

        def worker():
            self.tun_manager.connect(self._selected_server)

        threading.Thread(target=worker, daemon=True).start()

    def _start_disconnect(self):
        self._state = "disconnecting"
        self._set_status("connecting", "Отключение…")
        self._update_connect_button()

        def worker():
            self.tun_manager.disconnect()

        threading.Thread(target=worker, daemon=True).start()

    def _on_tun_connected(self):
        GLib.idle_add(self._apply_connected_state)

    def _apply_connected_state(self):
        self._state = "connected"
        name = self._selected_server["name"] if self._selected_server else ""
        self._set_status("connected", "Подключено", name)
        self._update_connect_button()
        if self.kill_switch_toggle.get_active():
            self._enable_kill_switch()
        self.stats.start("tun-vroxory")
        self.stats_bar.set_visible(True)
        return False

    def _on_tun_disconnected(self):
        GLib.idle_add(self._apply_disconnected_state)

    def _apply_disconnected_state(self):
        self._state = "idle"
        self._set_status("idle", "Не подключено")
        self._update_connect_button()
        self._disable_kill_switch()
        self.stats.stop()
        self.stats_bar.set_visible(False)
        return False

    def _on_tun_error(self, message: str):
        GLib.idle_add(self._apply_error_state, message)

    def _apply_error_state(self, message: str):
        self._state = "idle"
        self._set_status("error", "Ошибка")
        self._update_connect_button()
        self._show_banner(f"Ошибка: {message}", warning=True)
        self._disable_kill_switch()
        self.stats.stop()
        self.stats_bar.set_visible(False)
        return False

    def _on_stats_update(self, upload_bps: int, download_bps: int):
        GLib.idle_add(self.stats_bar.update, upload_bps, download_bps)

    def _on_tun_reconnecting(self, attempt: int, delay: float):
        GLib.idle_add(self._apply_reconnecting_state, attempt, delay)

    def _apply_reconnecting_state(self, attempt: int, delay: float):
        self._state = "connecting"
        self._set_status("connecting", f"Переподключение (попытка {attempt})…")
        self._update_connect_button()
        self.stats.stop()
        self.stats_bar.set_visible(False)
        return False

    def _on_tun_log(self, line: str):
        GLib.idle_add(self._log, line)

    def _log(self, line: str):
        print(f"[hysteria2] {line}")
        self.log_panel.append_line(line)
        return False
