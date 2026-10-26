//! Разбор одной ссылки `hysteria2://` (формат 3x-ui) в `Server`.

use std::collections::HashMap;

use percent_encoding::percent_decode_str;
use serde_json::Value as JsonValue;
use url::Url;

use super::Server;

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
        wireguard: None,
        manual_id: None,
    })
}
