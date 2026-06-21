//! Системный трей: значок + меню (подключить/отключить, выбор сервера,
//! открыть окно, выйти полностью). Список серверов в Rust не хранится
//! постоянно — фронтенд пушит актуальный список через sync_tray() при
//! любом изменении подписок/выбора (единственный источник правды —
//! React-состояние, оно же settings.json).

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::tray::{TrayIcon, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, Wry};

const TOGGLE_ID: &str = "tray_toggle";
const SHOW_ID: &str = "tray_show";
const QUIT_ID: &str = "tray_quit";
const SERVER_PREFIX: &str = "tray_server:";

pub fn build_tray(app: &AppHandle) -> tauri::Result<TrayIcon> {
    let menu = build_menu(app, false, None, &[])?;
    let mut builder = TrayIconBuilder::with_id("main")
        .menu(&menu)
        .tooltip("vrox.vpn")
        .on_menu_event(|app, event| handle_menu_event(app, event.id.as_ref()))
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { .. } = event {
                if let Some(window) = tray.app_handle().get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)
}

fn build_menu(
    app: &AppHandle,
    connected: bool,
    current_server: Option<&str>,
    servers: &[String],
) -> tauri::Result<Menu<Wry>> {
    let show = MenuItem::with_id(app, SHOW_ID, "Открыть", true, None::<&str>)?;
    let toggle_label = if connected { "Отключиться" } else { "Подключиться" };
    let toggle = MenuItem::with_id(app, TOGGLE_ID, toggle_label, true, None::<&str>)?;

    let server_items: Vec<MenuItem<Wry>> = servers
        .iter()
        .map(|name| {
            let id = format!("{SERVER_PREFIX}{name}");
            let label = if Some(name.as_str()) == current_server {
                format!("● {name}")
            } else {
                name.clone()
            };
            MenuItem::with_id(app, id, label, true, None::<&str>)
        })
        .collect::<tauri::Result<_>>()?;
    let server_refs: Vec<&dyn tauri::menu::IsMenuItem<Wry>> = server_items
        .iter()
        .map(|i| i as &dyn tauri::menu::IsMenuItem<Wry>)
        .collect();
    let servers_submenu = Submenu::with_items(app, "Серверы", true, &server_refs)?;

    let sep = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, QUIT_ID, "Выйти полностью", true, None::<&str>)?;

    Menu::with_items(app, &[&show, &toggle, &servers_submenu, &sep, &quit])
}

fn handle_menu_event(app: &AppHandle, id: &str) {
    match id {
        SHOW_ID => {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }
        TOGGLE_ID => {
            let _ = app.emit("tray-toggle-connection", ());
        }
        QUIT_ID => {
            app.exit(0);
        }
        other if other.starts_with(SERVER_PREFIX) => {
            let name = other.trim_start_matches(SERVER_PREFIX).to_string();
            let _ = app.emit("tray-select-server", name);
        }
        _ => {}
    }
}

#[tauri::command]
pub fn sync_tray(
    app: AppHandle,
    connected: bool,
    current_server: Option<String>,
    servers: Vec<String>,
) -> Result<(), String> {
    let menu = build_menu(&app, connected, current_server.as_deref(), &servers).map_err(|e| e.to_string())?;
    if let Some(tray) = app.tray_by_id("main") {
        tray.set_menu(Some(menu)).map_err(|e| e.to_string())?;
    }
    Ok(())
}
