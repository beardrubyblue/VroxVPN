//! CIDR-диапазоны IP-адресов России для обхода VPN (geoip-bypass) —
//! порт core/geoip.py (ветка main). Источник — ipverse/country-ip-blocks.

use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;
use tauri::AppHandle;

use crate::resources;

const SOURCE_IPV4: &str =
    "https://raw.githubusercontent.com/ipverse/country-ip-blocks/master/country/ru/ipv4-aggregated.txt";
const SOURCE_IPV6: &str =
    "https://raw.githubusercontent.com/ipverse/country-ip-blocks/master/country/ru/ipv6-aggregated.txt";

fn user_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".config/vroxory-vpn/geoip")
}

fn parse_cidr_file(path: &Path) -> Vec<String> {
    fs::read_to_string(path)
        .map(|content| {
            content
                .lines()
                .map(str::trim)
                .filter(|l| !l.is_empty() && !l.starts_with('#'))
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

/// Файл из user_dir (после "Обновить") приоритетнее встроенного снимка.
fn pick_path(user_file: PathBuf, bundled: PathBuf) -> PathBuf {
    if user_file.exists() {
        user_file
    } else {
        bundled
    }
}

pub fn get_ru_cidrs(app: &AppHandle) -> Result<(Vec<String>, Vec<String>), String> {
    let bundled_ipv4 = resources::resolve(app, "resources/geoip/ru_ipv4.txt")?;
    let bundled_ipv6 = resources::resolve(app, "resources/geoip/ru_ipv6.txt")?;
    let ipv4_path = pick_path(user_dir().join("ru_ipv4.txt"), bundled_ipv4);
    let ipv6_path = pick_path(user_dir().join("ru_ipv6.txt"), bundled_ipv6);
    Ok((parse_cidr_file(&ipv4_path), parse_cidr_file(&ipv6_path)))
}

#[derive(Serialize)]
pub struct UpdateResult {
    pub count: usize,
    pub bytes: usize,
}

pub async fn update_ru_cidrs() -> Result<UpdateResult, String> {
    let dir = user_dir();
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
