//! Tauri-команды, вызываемые фронтендом через invoke().

use serde::Serialize;
use tauri::{AppHandle, State};

use crate::app_update;
use crate::engine::{self, ActiveConnection, EngineState, Slot};
use crate::geoip;
use crate::geosite;
use crate::ping;
use crate::settings;
use crate::subscription::{self, Server};

#[derive(Serialize, Clone)]
pub struct ConnectionStatus {
    pub connected: bool,
    pub server_name: Option<String>,
}

#[tauri::command]
pub fn get_status(state: State<EngineState>) -> ConnectionStatus {
    let guard = state.0.lock().unwrap();
    match &*guard {
        Slot::Connected(conn) => ConnectionStatus {
            connected: true,
            server_name: Some(conn.server_name.clone()),
        },
        _ => ConnectionStatus {
            connected: false,
            server_name: None,
        },
    }
}

#[tauri::command]
pub async fn fetch_servers(url: String) -> Result<Vec<Server>, String> {
    let (servers, _userinfo) = subscription::fetch_subscription(&url, 15).await?;
    Ok(servers)
}

#[tauri::command]
pub async fn connect(
    app: AppHandle,
    state: State<'_, EngineState>,
    server: Server,
    ru_bypass: bool,
    kill_switch: bool,
) -> Result<(), String> {
    {
        let mut guard = state.0.lock().unwrap();
        match &*guard {
            Slot::Idle => *guard = Slot::Connecting,
            _ => return Err("уже подключено или подключение уже выполняется".into()),
        }
    }

    let result = connect_inner(&app, &server, ru_bypass).await;

    let mut guard = state.0.lock().unwrap();
    match result {
        Ok((handle, config_path)) => {
            *guard = Slot::Connected(ActiveConnection {
                handle,
                config_path,
                server_name: server.name,
            });
            drop(guard);
            if kill_switch {
                // best-effort: неудача kill switch не должна рвать уже
                // установленное VPN-соединение, только лишает доп. защиты
                if let Err(e) = engine::enable_killswitch(&app, &server.host) {
                    eprintln!("[killswitch] не удалось включить: {e}");
                }
            }
            Ok(())
        }
        Err(e) => {
            *guard = Slot::Idle;
            Err(e)
        }
    }
}

async fn connect_inner(
    app: &AppHandle,
    server: &Server,
    ru_bypass: bool,
) -> Result<(engine::ConnectionHandle, String), String> {
    // Генерация конфига (YAML-файл на Linux, JSON в памяти на macOS)
    // теперь внутри engine::spawn_client — платформо-специфична, не
    // общий контракт (см. docs/ARCHITECTURE.md, было открытым вопросом
    // до появления control-bridge на macOS).
    engine::ensure_polkit_rule(app)?;
    engine::loosen_rp_filter(app)?;
    engine::cleanup_interface(app);
    engine::spawn_client(app, server, ru_bypass).await
}

#[tauri::command]
pub async fn disconnect(app: AppHandle, state: State<'_, EngineState>) -> Result<(), String> {
    let conn = {
        let mut guard = state.0.lock().unwrap();
        match std::mem::replace(&mut *guard, Slot::Disconnecting) {
            Slot::Connected(conn) => conn,
            other => {
                *guard = other;
                return Err("не подключено".into());
            }
        }
    };

    match engine::kill_client(&app, &conn.config_path).await {
        Ok(()) => {
            // на Linux это обёртка pkexec-процесса (реальный root-процесс
            // hysteria2 уже убит выше); на macOS — `()`, no-op (Copy-тип,
            // `drop()` на нём — no-op с warning'ом компилятора, поэтому
            // `let _ =` вместо явного drop)
            let _ = conn.handle;
            *state.0.lock().unwrap() = Slot::Idle;
            // best-effort: если kill switch не был включён, это безвредный
            // no-op (см. disable_killswitch)
            engine::disable_killswitch(&app);
            Ok(())
        }
        Err(e) => {
            // kill не подтверждён — возвращаем состояние "подключено",
            // чтобы UI не показывал отключение, которое не произошло
            *state.0.lock().unwrap() = Slot::Connected(conn);
            Err(e)
        }
    }
}

/// Суммарный трафик с начала тоннеля (не дельта/скорость — это считает
/// фронтенд между двумя опросами, см. App.tsx). На Linux читается прямо
/// с `tun-vroxory` через `/proc/net/dev`, на macOS — через
/// `sendProviderMessage` к `.appex` (см. doc-комментарии
/// `engine::linux::get_traffic_totals`/`engine::macos::get_traffic_totals`).
/// Возвращает ошибку, если тоннель не активен — фронтенд должен сам не
/// опрашивать в это время (см. App.tsx, опрос только пока connected).
#[derive(Serialize, Clone)]
pub struct TrafficTotals {
    pub upload_bytes: u64,
    pub download_bytes: u64,
}

#[tauri::command]
pub async fn get_traffic_totals() -> Result<TrafficTotals, String> {
    let (upload_bytes, download_bytes) = engine::get_traffic_totals().await?;
    Ok(TrafficTotals {
        upload_bytes,
        download_bytes,
    })
}

#[tauri::command]
pub fn get_settings() -> serde_json::Value {
    serde_json::Value::Object(settings::load())
}

#[tauri::command]
pub fn set_setting(key: String, value: serde_json::Value) -> Result<(), String> {
    settings::set(&key, value)
}

#[tauri::command]
pub async fn ping_servers(servers: Vec<Server>) -> Vec<ping::PingResult> {
    let pairs = servers.into_iter().map(|s| (s.name, s.host)).collect();
    ping::ping_all(pairs).await
}

#[tauri::command]
pub async fn update_geoip() -> Result<geoip::UpdateResult, String> {
    geoip::update_ru_cidrs().await
}

#[tauri::command]
pub async fn update_geosite() -> Result<geosite::UpdateResult, String> {
    geosite::update_ru_domains().await
}

#[tauri::command]
pub async fn check_app_update() -> Result<app_update::UpdateCheck, String> {
    app_update::check_update(5).await
}

#[tauri::command]
pub fn quit_app(app: AppHandle) {
    app.exit(0);
}
