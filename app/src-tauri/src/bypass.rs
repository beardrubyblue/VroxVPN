//! RU-обход (geoip + geosite): пути приложения для vrox_core::bypass.
//! Сами списки, их разбор и обновление — в vrox-core, общие со шлюзом.

use std::path::PathBuf;

use tauri::{AppHandle, Manager};
use vrox_core::bypass::BypassDirs;
#[cfg(target_os = "linux")]
use vrox_core::{bypass, config_gen::RuBypass};

use crate::resources;

/// Куда пишутся скачанные списки (`geoip/`, `geosite/` внутри).
///
/// `dirs::home_dir()` + `.config/...` (как у Python-версии) ломается на
/// iOS: песочница не даёт просто так стучаться в произвольный путь под
/// HOME, `stat()` там валится с "Operation not permitted" — поймано
/// вживую при включении тоннеля с RU-bypass на реальном iPhone.
/// `app.path().app_data_dir()` — официальный портативный путь Tauri,
/// корректно резолвится в песочный контейнер на iOS и в
/// `~/.local/share`-эквивалент на Linux/macOS.
pub fn user_root(app: &AppHandle) -> PathBuf {
    app.path().app_data_dir().unwrap_or_else(|_| PathBuf::from("."))
}

/// Скачанные списки + встроенные снимки из ресурсов (tauri.conf.json,
/// bundle.resources: `resources/geoip/*`, `resources/geosite/*`).
pub fn dirs(app: &AppHandle) -> Result<BypassDirs, String> {
    Ok(BypassDirs {
        user_root: user_root(app),
        bundled_root: resources::resolve(app, "resources")?,
    })
}

/// Всё для YAML-конфига Linux: IP-диапазоны и домены (directDomains).
#[cfg(target_os = "linux")]
pub fn load(app: &AppHandle) -> Result<RuBypass, String> {
    let dirs = dirs(app)?;
    let (ipv4, ipv6) = bypass::load_ru_cidrs(&dirs);
    Ok(RuBypass { ipv4, ipv6, domains: bypass::load_ru_domains(&dirs) })
}
