//! Хранение настроек приложения в JSON файле — порт core/settings.py
//! (ветка main). Тот же путь, что у Python-приложения
//! (~/.config/vroxory-vpn/settings.json) — оба читают/пишут один файл,
//! merge поверх DEFAULTS сохраняет чужие ключи нетронутыми (например,
//! autostart_enabled, которого в Rust-версии пока нет).

use std::fs;
use std::path::PathBuf;

use serde_json::{json, Map, Value};
use tauri::{AppHandle, Manager};

/// `dirs::home_dir()` + `.config/...` ломается на iOS — App Sandbox не
/// даёт стучаться в произвольный путь под HOME, `fs::write`/`create_dir_all`
/// валятся с "Operation not permitted (os error 1)". Пойман вживую: эта
/// функция вызывается сразу после успешного connect (фронтенд сохраняет
/// last_selected_server), поэтому крash выглядел как "падает при
/// включении тоннеля". `app.path().app_data_dir()` — портативный путь
/// Tauri, тот же фикс, что и в geoip.rs/geosite.rs.
fn settings_dir(app: &AppHandle) -> PathBuf {
    app.path()
        .app_data_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
}

fn settings_path(app: &AppHandle) -> PathBuf {
    settings_dir(app).join("settings.json")
}

fn defaults() -> Map<String, Value> {
    let Value::Object(map) = json!({
        "subscription_url": "",
        "last_selected_server": "",
        "ru_bypass_enabled": false,
        "kill_switch_enabled": false,
        // Лимиты relay-слоя на iOS (packaging/hysteria2-patch/netunnel/
        // handler.go::applyRelayLimits) — настраиваемые из UI, дефолты
        // синхронизированы с Happ (другим клиентом, использован как
        // отправная точка для сравнения вживую), не с нашей прежней
        // эмпирикой (60с/48 — заметно туже, подбиралось отдельно под
        // нашу архитектуру общего мультиплексированного QUIC-туннеля).
        "idle_timeout_seconds": 300,
        "max_tcp_connections": 256,
        "max_udp_connections": 128,
    }) else {
        unreachable!()
    };
    map
}

pub fn load(app: &AppHandle) -> Map<String, Value> {
    let mut merged = defaults();
    if let Ok(content) = fs::read_to_string(settings_path(app)) {
        if let Ok(Value::Object(data)) = serde_json::from_str::<Value>(&content) {
            for (k, v) in data {
                merged.insert(k, v);
            }
        }
    }
    merged
}

pub fn save(app: &AppHandle, data: &Map<String, Value>) -> Result<(), String> {
    fs::create_dir_all(settings_dir(app)).map_err(|e| e.to_string())?;
    let text = serde_json::to_string_pretty(data).map_err(|e| e.to_string())?;
    fs::write(settings_path(app), text).map_err(|e| e.to_string())
}

pub fn set(app: &AppHandle, key: &str, value: Value) -> Result<(), String> {
    let mut data = load(app);
    data.insert(key.to_string(), value);
    save(app, &data)
}
