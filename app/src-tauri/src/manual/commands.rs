//! Tauri-команды узлов, добавленных вручную (регистрируются в lib.rs).

use tauri::AppHandle;

use super::store;
use vrox_core::subscription::Server;

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
/// (tauri-plugin-dialog, asCopy), поэтому читается обычным fs::read. На
/// Android приходит content:// URI — его читает fs-плагин через ContentResolver.
#[tauri::command]
pub async fn pick_config_file(app: AppHandle) -> Result<Option<String>, String> {
    #[cfg(mobile)]
    {
        use tauri_plugin_dialog::DialogExt;
        let picker = app.clone();
        let picked = tauri::async_runtime::spawn_blocking(move || picker.dialog().file().blocking_pick_file())
            .await
            .map_err(|e| e.to_string())?;
        let Some(file) = picked else {
            return Ok(None);
        };
        read_picked_file(&app, file).map(Some).map_err(|_| "Could not read the file".to_string())
    }
    #[cfg(not(mobile))]
    {
        let _ = app;
        Err("Choosing a file is only available on iOS and Android".into())
    }
}

#[cfg(target_os = "ios")]
fn read_picked_file(_app: &AppHandle, file: tauri_plugin_dialog::FilePath) -> Result<String, String> {
    let path = file.into_path().map_err(|e| e.to_string())?;
    std::fs::read_to_string(path).map_err(|e| e.to_string())
}

#[cfg(target_os = "android")]
fn read_picked_file(app: &AppHandle, file: tauri_plugin_dialog::FilePath) -> Result<String, String> {
    use tauri_plugin_fs::FsExt;
    app.fs().read_to_string(file).map_err(|e| e.to_string())
}
