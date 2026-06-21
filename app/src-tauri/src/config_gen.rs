//! Генерация YAML конфига hysteria2 для TUN-режима — порт
//! core/config_gen.py (ветка main) на Rust.

use std::net::{SocketAddr, ToSocketAddrs};
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
use std::path::PathBuf;

use serde_yaml::{Mapping, Value};
use tauri::AppHandle;

use crate::geoip;
use crate::geosite;
use crate::subscription::Server;

const CONFIG_DIR: &str = "/tmp/vroxory-vpn";

fn s(v: impl Into<String>) -> Value {
    Value::String(v.into())
}

fn safe_filename(name: &str) -> String {
    let mut result = String::new();
    let mut last_was_underscore = false;
    for c in name.chars() {
        let mapped = if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
            c
        } else {
            '_'
        };
        if mapped == '_' {
            if !last_was_underscore {
                result.push('_');
            }
            last_was_underscore = true;
        } else {
            result.push(mapped);
            last_was_underscore = false;
        }
    }
    let trimmed = result.trim_matches('_');
    if trimmed.is_empty() {
        "server".to_string()
    } else {
        trimmed.to_string()
    }
}

/// Создаёт CONFIG_DIR с правами 0700 (только текущий пользователь) —
/// конфиг внутри содержит пароль сервера в открытом виде. Отдельно
/// проверяет, что путь не подменили симлинком (классическая TOCTOU-
/// атака на предсказуемое имя в общем `/tmp`): если по этому пути
/// уже лежит симлинк — отказываемся следовать ему.
fn ensure_private_config_dir() -> Result<(), String> {
    if let Ok(meta) = std::fs::symlink_metadata(CONFIG_DIR) {
        if meta.file_type().is_symlink() {
            return Err(format!("{CONFIG_DIR} — симлинк, отказ"));
        }
        std::fs::set_permissions(CONFIG_DIR, std::fs::Permissions::from_mode(0o700))
            .map_err(|e| e.to_string())?;
        return Ok(());
    }
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(CONFIG_DIR)
        .map_err(|e| e.to_string())
}

/// Резолвит host в IPv4/IPv6 адреса — нужны для exclude-маршрутов, иначе
/// пакеты к самому VPN-серверу уйдут в TUN и получится routing loop.
fn resolve_server_addresses(host: &str) -> (Vec<String>, Vec<String>) {
    let mut ipv4 = Vec::new();
    let mut ipv6 = Vec::new();
    if let Ok(addrs) = format!("{host}:0").to_socket_addrs() {
        for addr in addrs {
            match addr {
                SocketAddr::V4(a) => {
                    let ip = a.ip().to_string();
                    if !ipv4.contains(&ip) {
                        ipv4.push(ip);
                    }
                }
                SocketAddr::V6(a) => {
                    let ip = a.ip().to_string();
                    if !ipv6.contains(&ip) {
                        ipv6.push(ip);
                    }
                }
            }
        }
    }
    (ipv4, ipv6)
}

fn str_seq(items: impl IntoIterator<Item = String>) -> Value {
    Value::Sequence(items.into_iter().map(Value::String).collect())
}

/// Генерирует YAML конфиг для сервера и возвращает путь к файлу.
///
/// `ru_bypass`: добавляет geoip-исключения IP-диапазонов России в
/// маршруты TUN и directDomains (geosite) для сайтов на зарубежном CDN,
/// которые под geoip не попадают — см. docs/ARCHITECTURE.md.
pub fn generate_config(
    app: &AppHandle,
    server: &Server,
    ru_bypass: bool,
) -> Result<PathBuf, String> {
    ensure_private_config_dir()?;

    let (server_ipv4, server_ipv6) = resolve_server_addresses(&server.host);

    let mut ipv4_exclude: Vec<String> = vec![
        "192.168.0.0/16".into(),
        "10.0.0.0/8".into(),
        "172.16.0.0/12".into(),
        "127.0.0.0/8".into(),
    ];
    ipv4_exclude.extend(server_ipv4.iter().map(|ip| format!("{ip}/32")));

    let mut ipv6_exclude: Vec<String> = vec!["fc00::/7".into(), "fe80::/10".into()];
    ipv6_exclude.extend(server_ipv6.iter().map(|ip| format!("{ip}/128")));

    let mut direct_domains: Vec<String> = Vec::new();
    if ru_bypass {
        let (ru_ipv4, ru_ipv6) = geoip::get_ru_cidrs(app)?;
        ipv4_exclude.extend(ru_ipv4);
        ipv6_exclude.extend(ru_ipv6);
        direct_domains = geosite::get_ru_domains(app)?;
    }

    let mut address = Mapping::new();
    address.insert(s("ipv4"), s("100.100.100.101/30"));
    address.insert(s("ipv6"), s("2001::ffff:ffff:ffff:fff1/126"));

    let mut route = Mapping::new();
    route.insert(s("ipv4"), str_seq(["0.0.0.0/0".to_string()]));
    route.insert(s("ipv6"), str_seq(["::/0".to_string()]));
    route.insert(s("ipv4Exclude"), str_seq(ipv4_exclude));
    route.insert(s("ipv6Exclude"), str_seq(ipv6_exclude));

    let mut tun = Mapping::new();
    tun.insert(s("name"), s("tun-vroxory"));
    tun.insert(s("mtu"), Value::Number(1500.into()));
    tun.insert(s("timeout"), Value::Number(300.into()));
    tun.insert(s("address"), Value::Mapping(address));
    tun.insert(s("route"), Value::Mapping(route));
    if !direct_domains.is_empty() {
        tun.insert(s("directDomains"), str_seq(direct_domains));
    }

    let sni = if server.sni.is_empty() {
        server.host.clone()
    } else {
        server.sni.clone()
    };
    let mut tls = Mapping::new();
    tls.insert(s("sni"), s(sni));
    if server.insecure {
        tls.insert(s("insecure"), Value::Bool(true));
    }
    if !server.pin_sha256.is_empty() {
        tls.insert(s("pinSHA256"), s(server.pin_sha256.clone()));
    }

    let mut bandwidth = Mapping::new();
    bandwidth.insert(s("up"), s("100 mbps"));
    bandwidth.insert(s("down"), s("100 mbps"));

    let mut config = Mapping::new();
    config.insert(s("server"), s(format!("{}:{}", server.host, server.port)));
    config.insert(s("auth"), s(server.password.clone()));
    config.insert(s("tls"), Value::Mapping(tls));
    config.insert(s("tun"), Value::Mapping(tun));
    config.insert(s("fastOpen"), Value::Bool(true));
    config.insert(s("bandwidth"), Value::Mapping(bandwidth));

    if !server.obfs.is_empty() {
        let mut salamander = Mapping::new();
        salamander.insert(s("password"), s(server.obfs_password.clone()));
        let mut obfs = Mapping::new();
        obfs.insert(s("type"), s(server.obfs.clone()));
        obfs.insert(s("salamander"), Value::Mapping(salamander));
        config.insert(s("obfs"), Value::Mapping(obfs));
    }

    if !server.quic.is_empty() {
        let quic_value = serde_yaml::to_value(&server.quic).map_err(|e| e.to_string())?;
        config.insert(s("quic"), quic_value);
    }

    // суффикс-хэш от host:port — без него два сервера, чьи имена совпадают
    // после санитизации (например "Server #1" и "Server_#1" оба дают
    // "Server_1"), перезаписывали бы конфиг друг друга
    let filename = format!(
        "{}_{:x}.yaml",
        safe_filename(&server.name),
        {
            use std::hash::{Hash, Hasher};
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            (&server.host, server.port).hash(&mut hasher);
            hasher.finish() & 0xffff
        }
    );
    let path = PathBuf::from(CONFIG_DIR).join(filename);
    let yaml_str = serde_yaml::to_string(&Value::Mapping(config)).map_err(|e| e.to_string())?;
    std::fs::write(&path, yaml_str).map_err(|e| e.to_string())?;
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))
        .map_err(|e| e.to_string())?;

    Ok(path)
}
