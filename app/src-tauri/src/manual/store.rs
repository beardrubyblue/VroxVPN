//! Узлы, добавленные вручную (группа «Added manually»): ссылка
//! `hysteria2://` или конфиг WireGuard/AmneziaWG (.conf). Храним исходный
//! текст — он и есть источник истины, протокол определяется при разборе.
//! Файл — в папке данных приложения (на iOS — песочница, шифруется
//! системой, пока устройство заблокировано).

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::subscription::{parse_hysteria2_uri, Server};
use crate::wireguard::parse_conf;

const FILE_NAME: &str = "manual_servers.json";
/// Раньше здесь хранились только WireGuard-конфиги — читаем, пока новый
/// файл не создан, чтобы уже импортированные конфиги не пропали.
const LEGACY_FILE_NAME: &str = "wireguard.json";

#[derive(Serialize, Deserialize, Clone)]
struct StoredServer {
    id: String,
    name: String,
    /// Исходный текст: ссылка hysteria2:// или .conf WireGuard. Поле
    /// называется `conf` ради совместимости со старым wireguard.json.
    conf: String,
}

fn data_dir(app: &AppHandle) -> Result<PathBuf, String> {
    app.path().app_data_dir().map_err(|e| e.to_string())
}

fn load(app: &AppHandle) -> Vec<StoredServer> {
    let Ok(dir) = data_dir(app) else {
        return Vec::new();
    };
    let read = |name: &str| std::fs::read(dir.join(name)).ok();
    read(FILE_NAME)
        .or_else(|| read(LEGACY_FILE_NAME))
        .and_then(|data| serde_json::from_slice(&data).ok())
        .unwrap_or_default()
}

fn save(app: &AppHandle, servers: &[StoredServer]) -> Result<(), String> {
    let dir = data_dir(app)?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join(FILE_NAME);
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_vec(servers).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &path).map_err(|e| e.to_string())
}

/// Разбор текста в узел: ссылка hysteria2:// или .conf WireGuard.
/// Ошибка — человеку понятная (показывается в уведомлении).
fn parse_server(text: &str) -> Result<Server, String> {
    let text = text.trim();
    if text.starts_with("hysteria2://") {
        return parse_hysteria2_uri(text);
    }
    if !text.contains("[Interface]") {
        return Err("Paste a hysteria2:// link or a WireGuard config".into());
    }
    // WireGuard на Linux пока не поддерживается — не даём добавить узел,
    // к которому потом нельзя подключиться
    if cfg!(target_os = "linux") {
        return Err("WireGuard is not supported on Linux yet".into());
    }
    let profile = parse_conf(text)?;
    let (host, port) = profile.endpoint_host_port()?;
    Ok(Server { host, port, wireguard: Some(profile), ..Server::default() })
}

fn to_server(stored: &StoredServer) -> Option<Server> {
    let mut server = parse_server(&stored.conf).ok()?;
    server.name = stored.name.clone();
    server.manual_id = Some(stored.id.clone());
    // raw_uri уникален — по нему строится ключ строки в UI
    server.raw_uri = format!("manual:{}", stored.id);
    Some(server)
}

pub fn list(app: &AppHandle) -> Vec<Server> {
    load(app).iter().filter_map(to_server).collect()
}

/// Проверяет текст и сохраняет. Пустое имя — имя из ссылки/хост сервера.
pub fn import(app: &AppHandle, name: &str, text: &str) -> Result<Server, String> {
    let parsed = parse_server(text)?;
    let name = if name.trim().is_empty() { fallback_name(&parsed) } else { name.trim().to_string() };
    let id = SystemTime::now().duration_since(UNIX_EPOCH).map_err(|e| e.to_string())?.as_nanos();
    let stored = StoredServer { id: format!("{id:x}"), name, conf: text.trim().to_string() };
    let mut servers = load(app);
    servers.push(stored.clone());
    save(app, &servers)?;
    to_server(&stored).ok_or_else(|| "Could not read the saved server".to_string())
}

fn fallback_name(server: &Server) -> String {
    if server.name.is_empty() { server.host.clone() } else { server.name.clone() }
}

pub fn rename(app: &AppHandle, id: &str, name: &str) -> Result<(), String> {
    let mut servers = load(app);
    let server = servers.iter_mut().find(|server| server.id == id).ok_or("Server not found")?;
    server.name = name.trim().to_string();
    save(app, &servers)
}

pub fn remove(app: &AppHandle, id: &str) -> Result<(), String> {
    let mut servers = load(app);
    servers.retain(|server| server.id != id);
    save(app, &servers)
}

#[cfg(test)]
mod tests {
    use super::parse_server;

    #[test]
    fn detects_protocol_from_text() {
        let hysteria = parse_server("  hysteria2://secret@vpn.example.com:443/?sni=vpn.example.com#DE\n").unwrap();
        assert!(hysteria.wireguard.is_none());
        assert_eq!(hysteria.host, "vpn.example.com");

        let conf = "[Interface]\nPrivateKey = cHJpdg==\nAddress = 10.8.0.2/24\n\
                    [Peer]\nPublicKey = cHVi\nAllowedIPs = 0.0.0.0/0\nEndpoint = 192.0.2.1:51820\n";
        #[cfg(not(target_os = "linux"))]
        assert!(parse_server(conf).unwrap().wireguard.is_some());
        #[cfg(target_os = "linux")]
        assert!(parse_server(conf).is_err());

        assert!(parse_server("https://example.com/sub").is_err());
    }
}
