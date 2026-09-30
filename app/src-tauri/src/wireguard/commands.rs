//! Tauri-команды WireGuard для фронтенда (регистрируются в lib.rs).

use tauri::AppHandle;

use super::store;
use crate::subscription::Server;

#[tauri::command]
pub fn list_wireguard(app: AppHandle) -> Vec<Server> {
    store::list(&app)
}

/// `conf` — текст .conf: вставленный, прочитанный из файла или из QR.
#[tauri::command]
pub fn import_wireguard(app: AppHandle, name: String, conf: String) -> Result<Server, String> {
    store::import(&app, &name, &conf)
}

#[tauri::command]
pub fn rename_wireguard(app: AppHandle, id: String, name: String) -> Result<(), String> {
    store::rename(&app, &id, &name)
}

#[tauri::command]
pub fn delete_wireguard(app: AppHandle, id: String) -> Result<(), String> {
    store::remove(&app, &id)
}
