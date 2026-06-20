//! Загрузка и парсинг подписки hysteria2 — порт core/subscription.py
//! (ветка main) на Rust, тот же контракт и те же поля.

use std::collections::HashMap;

use base64::{engine::general_purpose::STANDARD, Engine as _};
use percent_encoding::percent_decode_str;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use url::Url;

// 3x-ui кладёт QUIC-тюнинг в quicParams (формат xray-core "finalmask"), но
// часть имён полей у бинарника hysteria2 называется иначе — маппинг ниже.
const QUIC_FIELD_MAP: &[(&str, &str)] = &[
    ("initStreamReceiveWindow", "initStreamReceiveWindow"),
    ("maxStreamReceiveWindow", "maxStreamReceiveWindow"),
    ("initConnectionReceiveWindow", "initConnReceiveWindow"),
    ("maxConnectionReceiveWindow", "maxConnReceiveWindow"),
    ("maxIdleTimeout", "maxIdleTimeout"),
    ("keepAlivePeriod", "keepAlivePeriod"),
    ("disablePathMTUDiscovery", "disablePathMTUDiscovery"),
];

// Эти поля в 3x-ui хранятся как целые секунды, а в hysteria2 quic: ожидают
// строку вида "30s" — голое число hysteria2 распарсит как наносекунды.
const QUIC_DURATION_FIELDS: &[&str] = &["maxIdleTimeout", "keepAlivePeriod"];

#[derive(Serialize, Deserialize, Clone, Debug)]
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
}

#[derive(Serialize, Clone, Debug, Default)]
pub struct UserInfo {
    pub fields: HashMap<String, i64>,
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

fn parse_quic_params(fm_raw: &str) -> HashMap<String, JsonValue> {
    let mut result = HashMap::new();
    if fm_raw.is_empty() {
        return result;
    }
    let finalmask: JsonValue = match serde_json::from_str(fm_raw) {
        Ok(v) => v,
        Err(_) => return result,
    };
    let quic_params = match finalmask.get("quicParams").and_then(JsonValue::as_object) {
        Some(m) => m,
        None => return result,
    };
    for (sub_key, hysteria_key) in QUIC_FIELD_MAP {
        let Some(value) = quic_params.get(*sub_key) else {
            continue;
        };
        let value = if QUIC_DURATION_FIELDS.contains(hysteria_key) {
            let seconds = value
                .as_i64()
                .map(|n| n.to_string())
                .unwrap_or_else(|| value.to_string());
            JsonValue::String(format!("{seconds}s"))
        } else {
            value.clone()
        };
        result.insert(hysteria_key.to_string(), value);
    }
    result
}

fn try_base64_decode(text: &str) -> String {
    let mut padded = text.to_string();
    while padded.len() % 4 != 0 {
        padded.push('=');
    }
    match STANDARD.decode(&padded) {
        Ok(bytes) => String::from_utf8_lossy(&bytes).to_string(),
        Err(_) => text.to_string(),
    }
}

pub fn parse_hysteria2_uri(uri: &str) -> Result<Server, String> {
    if !uri.starts_with("hysteria2://") {
        return Err("Not a hysteria2 URI".into());
    }
    let parsed = Url::parse(uri).map_err(|e| e.to_string())?;

    let password = percent_decode_str(parsed.username())
        .decode_utf8_lossy()
        .to_string();
    let host = parsed
        .host_str()
        .ok_or("Missing host in hysteria2 URI")?
        .to_string();
    let port = parsed.port().ok_or("Missing port in hysteria2 URI")?;

    let name = if let Some(fragment) = parsed.fragment() {
        percent_decode_str(fragment).decode_utf8_lossy().to_string()
    } else {
        format!("{host}:{port}")
    };

    let query: HashMap<String, String> = parsed.query_pairs().into_owned().collect();
    let first = |key: &str| -> String { query.get(key).cloned().unwrap_or_default() };

    let insecure_raw = first("insecure").to_lowercase();
    let insecure = matches!(insecure_raw.as_str(), "1" | "true" | "yes");

    Ok(Server {
        name,
        host: host.clone(),
        port,
        password,
        sni: {
            let sni = first("sni");
            if sni.is_empty() { host } else { sni }
        },
        insecure,
        obfs: first("obfs"),
        obfs_password: first("obfs-password"),
        pin_sha256: first("pinSHA256"),
        quic: parse_quic_params(&first("fm")),
        raw_uri: uri.to_string(),
    })
}
