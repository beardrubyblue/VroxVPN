//! CIDR-диапазоны IP-адресов России для обхода VPN (geoip-bypass) —
//! порт core/geoip.py (ветка main). Источник — ipverse/country-ip-blocks.

use std::fs;
use std::path::Path;

use super::{parse_list_file, pick_path, BypassDirs, UpdateResult};

const DIR: &str = "geoip";
const SOURCE_IPV4: &str =
    "https://raw.githubusercontent.com/ipverse/country-ip-blocks/master/country/ru/ipv4-aggregated.txt";
const SOURCE_IPV6: &str =
    "https://raw.githubusercontent.com/ipverse/country-ip-blocks/master/country/ru/ipv6-aggregated.txt";

/// (IPv4, IPv6) CIDR России: скачанные через `update_ru_cidrs`, иначе
/// встроенный снимок.
pub fn load_ru_cidrs(dirs: &BypassDirs) -> (Vec<String>, Vec<String>) {
    let load = |name: &str| {
        let path = pick_path(&dirs.user_root, &dirs.bundled_root, DIR, name);
        parse_list_file(&path)
    };
    (load("ru_ipv4.txt"), load("ru_ipv6.txt"))
}

/// Скачивает свежие списки в `<user_root>/geoip/` — они приоритетнее
/// встроенного снимка при следующем `load_ru_cidrs`.
pub async fn update_ru_cidrs(user_root: &Path) -> Result<UpdateResult, String> {
    let dir = user_root.join(DIR);
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    let client = reqwest::Client::new();
    let mut total_count = 0;
    let mut total_bytes = 0;

    for (url, filename) in [(SOURCE_IPV4, "ru_ipv4.txt"), (SOURCE_IPV6, "ru_ipv6.txt")] {
        let resp = client
            .get(url)
            .timeout(std::time::Duration::from_secs(20))
            .send()
            .await
            .map_err(|e| e.to_string())?
            .error_for_status()
            .map_err(|e| e.to_string())?;
        let bytes = resp.bytes().await.map_err(|e| e.to_string())?;
        let text = String::from_utf8_lossy(&bytes).to_string();
        let count = text
            .lines()
            .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
            .count();
        if count == 0 {
            return Err(format!("Пустой ответ при обновлении базы {filename}"));
        }
        fs::write(dir.join(filename), &text).map_err(|e| e.to_string())?;
        total_count += count;
        total_bytes += bytes.len();
    }

    Ok(UpdateResult {
        count: total_count,
        bytes: total_bytes,
    })
}
