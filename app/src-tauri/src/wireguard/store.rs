//! Хранение импортированных WireGuard-конфигов: `wireguard.json` в папке
//! данных приложения (на iOS — песочница приложения, шифруется системой,
//! пока устройство заблокировано). Храним исходный текст .conf — он и
//! есть источник истины, разбирается при чтении.

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use super::parse_conf;
use crate::subscription::Server;

const FILE_NAME: &str = "wireguard.json";

#[derive(Serialize, Deserialize, Clone)]
struct StoredConfig {
    id: String,
    name: String,
    conf: String,
}

fn store_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    Ok(dir.join(FILE_NAME))
}

fn load(app: &AppHandle) -> Vec<StoredConfig> {
    store_path(app)
        .ok()
        .and_then(|path| std::fs::read(path).ok())
        .and_then(|data| serde_json::from_slice(&data).ok())
        .unwrap_or_default()
}

fn save(app: &AppHandle, configs: &[StoredConfig]) -> Result<(), String> {
    let path = store_path(app)?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_vec(configs).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &path).map_err(|e| e.to_string())
}

/// WireGuard-узел как обычный `Server`: hysteria-поля пустые, всё нужное
/// тоннелю — в `wireguard`. raw_uri уникален — по нему строится ключ строки в UI.
fn to_server(stored: &StoredConfig) -> Option<Server> {
    let mut profile = parse_conf(&stored.conf).ok()?;
    profile.id = stored.id.clone();
    let (host, port) = profile.endpoint_host_port().ok()?;
    Some(Server {
        name: stored.name.clone(),
        host,
        port,
        raw_uri: format!("wireguard:{}", stored.id),
        wireguard: Some(profile),
        ..Server::default()
    })
}

pub fn list(app: &AppHandle) -> Vec<Server> {
    load(app).iter().filter_map(to_server).collect()
}

/// Проверяет конфиг (ошибка — человеку понятная) и сохраняет.
pub fn import(app: &AppHandle, name: &str, conf: &str) -> Result<Server, String> {
    parse_conf(conf)?;
    let id = SystemTime::now().duration_since(UNIX_EPOCH).map_err(|e| e.to_string())?.as_nanos();
    let stored = StoredConfig { id: format!("{id:x}"), name: name.trim().to_string(), conf: conf.to_string() };
    let mut configs = load(app);
    configs.push(stored.clone());
    save(app, &configs)?;
    to_server(&stored).ok_or_else(|| "Не удалось прочитать сохранённый конфиг".to_string())
}

pub fn rename(app: &AppHandle, id: &str, name: &str) -> Result<(), String> {
    let mut configs = load(app);
    let config = configs.iter_mut().find(|config| config.id == id).ok_or("Конфиг не найден")?;
    config.name = name.trim().to_string();
    save(app, &configs)
}

pub fn remove(app: &AppHandle, id: &str) -> Result<(), String> {
    let mut configs = load(app);
    configs.retain(|config| config.id != id);
    save(app, &configs)
}
