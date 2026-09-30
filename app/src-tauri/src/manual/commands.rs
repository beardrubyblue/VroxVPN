//! Tauri-команды узлов, добавленных вручную (регистрируются в lib.rs).

use tauri::AppHandle;

use super::store;
use crate::subscription::Server;

#[tauri::command]
pub fn list_manual_servers(app: AppHandle) -> Vec<Server> {
    store::list(&app)
}

/// `text` — ссылка hysteria2:// или .conf WireGuard: вставленный текст,
/// файл или QR. Пустое `name` — имя из ссылки / хост сервера.
#[tauri::command]
pub fn import_manual_server(app: AppHandle, name: String, text: String) -> Result<Server, String> {
    store::import(&app, &name, &text)
}

#[tauri::command]
pub fn rename_manual_server(app: AppHandle, id: String, name: String) -> Result<(), String> {
    store::rename(&app, &id, &name)
}

#[tauri::command]
pub fn delete_manual_server(app: AppHandle, id: String) -> Result<(), String> {
    store::remove(&app, &id)
}

/// Выбрать файл конфига в «Файлах» и вернуть его текст; `None` — выбор
/// отменён. На iOS выбранный файл копируется во временную папку приложения
/// (tauri-plugin-dialog, asCopy), поэтому читается обычным fs::read.
#[tauri::command]
pub async fn pick_config_file(app: AppHandle) -> Result<Option<String>, String> {
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
        std::fs::read_to_string(path).map(Some).map_err(|_| "Could not read the file".to_string())
    }
    #[cfg(not(target_os = "ios"))]
    {
        let _ = app;
        Err("Choosing a file is only available on iOS".into())
    }
}
