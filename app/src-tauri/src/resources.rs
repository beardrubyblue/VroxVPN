//! Резолвинг путей к бандленным ресурсам (privileged_helper.sh,
//! встроенные снимки geoip/geosite) — единое место, которое работает
//! и в `tauri dev`, и в собранном/установленном приложении. Список
//! ресурсов и их относительные пути объявлены в tauri.conf.json
//! (bundle.resources) — путь, который сюда передаётся, должен совпадать
//! с путём оттуда.

use std::path::PathBuf;

use tauri::path::BaseDirectory;
use tauri::{AppHandle, Manager};

pub fn resolve(app: &AppHandle, relative: &str) -> Result<PathBuf, String> {
    app.path()
        .resolve(relative, BaseDirectory::Resource)
        .map_err(|e| format!("не удалось найти ресурс {relative}: {e}"))
}
