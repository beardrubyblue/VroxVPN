//! Хранение настроек приложения в JSON файле — порт core/settings.py
//! (ветка main). Тот же путь, что у Python-приложения
//! (~/.config/vroxory-vpn/settings.json) — оба читают/пишут один файл,
//! merge поверх DEFAULTS сохраняет чужие ключи нетронутыми (kill_switch_
//! enabled, autostart_enabled и т.п., которых в Rust-версии пока нет).

use std::fs;
use std::path::PathBuf;

use serde_json::{json, Map, Value};

fn settings_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".config/vroxory-vpn")
}

fn settings_path() -> PathBuf {
    settings_dir().join("settings.json")
}

fn defaults() -> Map<String, Value> {
    let Value::Object(map) = json!({
        "subscription_url": "",
        "last_selected_server": "",
        "ru_bypass_enabled": false,
    }) else {
        unreachable!()
    };
    map
}

pub fn load() -> Map<String, Value> {
    let mut merged = defaults();
    if let Ok(content) = fs::read_to_string(settings_path()) {
        if let Ok(Value::Object(data)) = serde_json::from_str::<Value>(&content) {
            for (k, v) in data {
                merged.insert(k, v);
            }
        }
    }
    merged
}

pub fn save(data: &Map<String, Value>) -> Result<(), String> {
    fs::create_dir_all(settings_dir()).map_err(|e| e.to_string())?;
    let text = serde_json::to_string_pretty(data).map_err(|e| e.to_string())?;
    fs::write(settings_path(), text).map_err(|e| e.to_string())
}

pub fn set(key: &str, value: Value) -> Result<(), String> {
    let mut data = load();
    data.insert(key.to_string(), value);
    save(&data)
}
