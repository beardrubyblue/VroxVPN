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

/// Выбрать .conf в «Файлах» и вернуть его текст; `None` — выбор отменён.
/// На iOS выбранный файл копируется во временную папку приложения
/// (tauri-plugin-dialog, asCopy), поэтому читается обычным fs::read.
#[tauri::command]
pub async fn pick_wireguard_file(app: AppHandle) -> Result<Option<String>, String> {
    #[cfg(target_os = "ios")]
    {
        use tauri_plugin_dialog::DialogExt;
        let picked = tauri::async_runtime::spawn_blocking(move || app.dialog().file().blocking_pick_file())
            .await
            .map_err(|e| e.to_string())?;
        let Some(file) = picked else {
            return Ok(None);
        };
        let path = file.into_path().map_err(|e| e.to_string())?;
        std::fs::read_to_string(path).map(Some).map_err(|_| "Не удалось прочитать файл".to_string())
    }
    #[cfg(not(target_os = "ios"))]
    {
        let _ = app;
        Err("Импорт файла WireGuard доступен только на iOS".into())
    }
}
