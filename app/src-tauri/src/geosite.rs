//! Список доменов российских сервисов для обхода VPN (geosite-bypass) —
//! порт core/geosite.py (ветка main). Источник —
//! v2fly/domain-list-community, category-ru с рекурсивными include:.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;
use tauri::AppHandle;

use crate::resources;

const SOURCE_BASE: &str = "https://raw.githubusercontent.com/v2fly/domain-list-community/master/data/";
const ROOT_CATEGORY: &str = "category-ru";

fn user_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".config/vroxory-vpn/geosite")
}

fn parse_domain_file(path: &Path) -> Vec<String> {
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

pub fn get_ru_domains(app: &AppHandle) -> Result<Vec<String>, String> {
    let user_file = user_dir().join("ru_domains.txt");
    let path = if user_file.exists() {
        user_file
    } else {
        resources::resolve(app, "resources/geosite/ru_domains.txt")?
    };
    Ok(parse_domain_file(&path))
}

#[derive(Serialize)]
pub struct UpdateResult {
    pub count: usize,
    pub bytes: usize,
}

async fn fetch_file(client: &reqwest::Client, name: &str, retries: u32) -> Option<String> {
    let url = format!("{SOURCE_BASE}{name}");
    for _ in 0..retries {
        let Ok(resp) = client
            .get(&url)
            .timeout(std::time::Duration::from_secs(15))
            .send()
            .await
        else {
            continue;
        };
        let Ok(resp) = resp.error_for_status() else {
            continue;
        };
        if let Ok(text) = resp.text().await {
            return Some(text);
        }
    }
    None
}

pub async fn update_ru_domains() -> Result<UpdateResult, String> {
    let client = reqwest::Client::new();
    let mut seen_files: HashSet<String> = HashSet::new();
    let mut domains: HashSet<String> = HashSet::new();
    let mut pending: HashSet<String> = HashSet::from([ROOT_CATEGORY.to_string()]);

    while !pending.is_empty() {
        let to_fetch: Vec<String> = pending
            .iter()
            .filter(|n| !seen_files.contains(*n))
            .cloned()
            .collect();
        seen_files.extend(to_fetch.iter().cloned());
        pending.clear();
        if to_fetch.is_empty() {
            break;
        }

        let fetches = to_fetch.iter().map(|name| {
            let client = client.clone();
            let name = name.clone();
            async move {
                let text = fetch_file(&client, &name, 2).await;
                (name, text)
            }
        });
        let results = futures::future::join_all(fetches).await;

        for (_name, text) in results {
            let Some(text) = text else { continue };
            for raw_line in text.lines() {
                let line = raw_line.trim();
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }
                let line = line.split('#').next().unwrap_or("").trim();
                if line.is_empty() {
                    continue;
                }
                if let Some(inc) = line.strip_prefix("include:") {
                    let inc = inc.trim().to_string();
                    if !seen_files.contains(&inc) {
                        pending.insert(inc);
                    }
                    continue;
                }
                let mut entry = line.split('@').next().unwrap_or("").trim();
                for prefix in ["full:", "domain:"] {
                    if let Some(stripped) = entry.strip_prefix(prefix) {
                        entry = stripped;
                        break;
                    }
                }
                if entry.is_empty() || entry.starts_with("regexp:") || entry.starts_with("keyword:") {
                    continue;
                }
                domains.insert(entry.to_lowercase());
            }
        }
    }

    if domains.is_empty() {
        return Err("Не удалось скачать ни одного домена из category-ru".into());
    }

    let dir = user_dir();
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let mut sorted: Vec<String> = domains.into_iter().collect();
    sorted.sort();
    let text = sorted.join("\n") + "\n";
    fs::write(dir.join("ru_domains.txt"), &text).map_err(|e| e.to_string())?;

    Ok(UpdateResult {
        count: sorted.len(),
        bytes: text.len(),
    })
}
