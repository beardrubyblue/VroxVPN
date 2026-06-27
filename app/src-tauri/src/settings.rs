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
        // handler.go::applyRelayLimits). Агрессивные дефолты для iOS NE
        // (~50 МиБ jetsam): 30с idle вместо 300с Happ — при листании
        // Reels 5-минутный таймаут копил сотни соединений по ~200 КиБ
        // каждое (gVisor-буфера + io.Copy + goroutine stacks); 64/32
        // relay вместо 256/128 — потолок живых соединений, а не скорость.
        "idle_timeout_seconds": 30,
        "max_tcp_connections": 64,
        "max_udp_connections": 32,
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
    migrate(app, &mut merged);
    merged
}

/// Одноразовые миграции — старые дефолты relay-лимитов (300/256/128,
/// подобранные по Happ) оказались слишком высокими для iOS NE: 300с
/// idle копил сотни TCP-соединений при листании Reels, jetsam убивал
/// расширение за ~7 мин. Если в сохранённых настройках лежат ровно
/// старые дефолты (пользователь их сам не менял), обновляем до новых.
fn migrate(app: &AppHandle, settings: &mut Map<String, Value>) {
    let old_idle = settings.get("idle_timeout_seconds") == Some(&json!(300));
    let old_tcp = settings.get("max_tcp_connections") == Some(&json!(256));
    let old_udp = settings.get("max_udp_connections") == Some(&json!(128));
    if old_idle && old_tcp && old_udp {
        settings.insert("idle_timeout_seconds".into(), json!(30));
        settings.insert("max_tcp_connections".into(), json!(64));
        settings.insert("max_udp_connections".into(), json!(32));
        let _ = save(app, settings);
    }
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
