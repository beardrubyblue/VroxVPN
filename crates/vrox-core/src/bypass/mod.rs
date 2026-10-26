//! RU-обход: IP-диапазоны (geoip) и домены (geosite) России, которые идут
//! мимо тоннеля. У каждого списка два источника: встроенный снимок
//! (`bundled_root`, ресурсы приложения / `/usr/share/vroxgw`) и скачанный
//! кнопкой «Обновить» или таймером шлюза (`user_root`) — второй приоритетнее.

mod geoip;
mod geosite;

use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;

pub use geoip::{load_ru_cidrs, update_ru_cidrs};
pub use geosite::{load_ru_domains, update_ru_domains};

/// Корни, внутри которых лежат `geoip/` и `geosite/`.
pub struct BypassDirs {
    pub user_root: PathBuf,
    pub bundled_root: PathBuf,
}

#[derive(Serialize)]
pub struct UpdateResult {
    pub count: usize,
    pub bytes: usize,
}

/// Файл из user_root (после обновления) приоритетнее встроенного снимка.
fn pick_path(user_root: &Path, bundled_root: &Path, dir: &str, name: &str) -> PathBuf {
    let user_file = user_root.join(dir).join(name);
    if user_file.exists() {
        user_file
    } else {
        bundled_root.join(dir).join(name)
    }
}

/// Строки файла без пустых и `#`-комментариев; нет файла — пустой список.
fn parse_list_file(path: &Path) -> Vec<String> {
    fs::read_to_string(path)
        .map(|content| {
            content
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty() && !line.starts_with('#'))
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}
