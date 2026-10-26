//! Загрузка и парсинг подписки hysteria2 — порт core/subscription.py
//! (ветка main) на Rust, тот же контракт и те же поля.

mod uri;

use std::collections::HashMap;

use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

pub use uri::parse_hysteria2_uri;

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Server {
    pub name: String,
    pub host: String,
    pub port: u16,
    pub password: String,
    pub sni: String,
    pub insecure: bool,
    pub obfs: String,
    pub obfs_password: String,
    pub pin_sha256: String,
    pub quic: HashMap<String, JsonValue>,
    pub raw_uri: String,
    /// Есть — WireGuard/AmneziaWG-узел (импортированный .conf, см.
    /// wireguard/), нет — hysteria2 из подписки. Остальные поля у
    /// WireGuard-узла пустые, кроме name/host/port/raw_uri.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wireguard: Option<crate::wireguard::WireGuardProfile>,
    /// Есть — узел добавлен вручную (manual/: ссылка hysteria2:// или
    /// .conf WireGuard), это id записи для переименования/удаления; нет —
    /// узел из подписки.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub manual_id: Option<String>,
}

#[derive(Serialize, Clone, Debug, Default)]
pub struct UserInfo {
    pub fields: HashMap<String, i64>,
}

/// Ответ `fetch_subscription` фронтенду: серверы + `Subscription-Userinfo`
/// (upload/download/total/expire). Раньше userinfo парсился и
/// выбрасывался — теперь он нужен для строки трафика/срока в заголовке
/// группы подписки на экране Nodes.
#[derive(Serialize, Clone, Debug)]
pub struct SubscriptionData {
    pub servers: Vec<Server>,
    pub userinfo: UserInfo,
}

pub async fn fetch_subscription(url: &str, timeout_secs: u64) -> Result<(Vec<Server>, UserInfo), String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(timeout_secs))
        .build()
        .map_err(|e| e.to_string())?;
    let resp = client.get(url).send().await.map_err(|e| e.to_string())?;
    let resp = resp.error_for_status().map_err(|e| e.to_string())?;

    let userinfo = resp
        .headers()
        .get("Subscription-Userinfo")
        .and_then(|v| v.to_str().ok())
        .map(parse_userinfo)
        .unwrap_or_default();

    let mut text = resp.text().await.map_err(|e| e.to_string())?;
    text = text.trim().to_string();

    if !text.starts_with("hysteria2://") {
        text = try_base64_decode(&text);
    }

    let servers = text
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with("hysteria2://"))
        .filter_map(|line| parse_hysteria2_uri(line).ok())
        .collect();

    Ok((servers, userinfo))
}

fn parse_userinfo(header_value: &str) -> UserInfo {
    let mut fields = HashMap::new();
    for part in header_value.split(';') {
        let part = part.trim();
        if let Some((key, value)) = part.split_once('=') {
            if let Ok(n) = value.trim().parse::<i64>() {
                fields.insert(key.trim().to_string(), n);
            }
        }
    }
    UserInfo { fields }
}

fn try_base64_decode(text: &str) -> String {
    let mut padded = text.to_string();
    while !padded.len().is_multiple_of(4) {
        padded.push('=');
    }
    match STANDARD.decode(&padded) {
        Ok(bytes) => String::from_utf8_lossy(&bytes).to_string(),
        Err(_) => text.to_string(),
    }
}
