//! Генерация YAML конфига hysteria2 для TUN-режима — порт
//! core/config_gen.py (ветка main) на Rust.

use std::net::{SocketAddr, ToSocketAddrs};
#[cfg(target_os = "linux")]
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
#[cfg(target_os = "linux")]
use std::path::PathBuf;

#[cfg(target_os = "linux")]
use serde_yaml::{Mapping, Value};
use tauri::AppHandle;

use crate::geoip;
#[cfg(target_os = "linux")]
use crate::geosite;
use crate::subscription::Server;

#[cfg(target_os = "linux")]
const CONFIG_DIR: &str = "/tmp/vroxory-vpn";

#[cfg(target_os = "linux")]
fn s(v: impl Into<String>) -> Value {
    Value::String(v.into())
}

#[cfg(target_os = "linux")]
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
#[cfg(target_os = "linux")]
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

#[cfg(target_os = "linux")]
fn str_seq(items: impl IntoIterator<Item = String>) -> Value {
    Value::Sequence(items.into_iter().map(Value::String).collect())
}

/// Генерирует YAML конфиг для сервера и возвращает путь к файлу.
///
/// `ru_bypass`: добавляет geoip-исключения IP-диапазонов России в
/// маршруты TUN и directDomains (geosite) для сайтов на зарубежном CDN,
/// которые под geoip не попадают — см. docs/ARCHITECTURE.md.
#[cfg(target_os = "linux")]
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

/// CIDR-диапазоны, которые НЕ должны идти в тоннель — аналог
/// `route.ipv4Exclude`/`ipv6Exclude` из YAML выше, но для будущего NE-пути
/// (macOS/iOS): там это не настройка sing-tun внутри hysteria2-клиента, а
/// `NEPacketTunnelNetworkSettings.excludedRoutes` на стороне Swift, до
/// того как тоннель поднят — поэтому отдельная функция, а не общий код с
/// `generate_config`.
///
/// ⚠ Сюда сознательно НЕ включены домены из `geosite::get_ru_domains`
/// (~1736 штук) — первая версия плана (см. git-историю ARCHITECTURE.md)
/// предполагала резолвить их все в IP заранее, при генерации конфига.
/// Проверка на месте показала: это плохая идея — 1736 блокирующих DNS-
/// резолвов на каждый connect означало бы реальные секунды/десятки
/// секунд задержки и кучу таймаутов на доменах, которые объект не
/// резолвит напрямую (некоторые записи geosite — суффиксы, не сами
/// резолвящиеся имена). Live-сниффинг (как на Linux, `dnssniff_linux.go`)
/// под NE, скорее всего, переезжает не сюда, а в `relayHandler.
/// NewPacketConnection` самого `netunnel` — там UDP-трафик к порту 53 и
/// так проходит через наш relay (NE не вырезает DNS из тоннеля так, как
/// это делает sing-tun AutoRoute на Linux), значит дозвон до резолвера
/// и ответ уже виден нашему коду без отдельного AF_PACKET-сниффера. Но
/// чтобы реально исключить разрешённый IP из тоннеля ПОСЛЕ старта, нужен
/// способ сообщить об этом обратно в Swift (повторный вызов
/// `setTunnelNetworkSettings` с обновлённым `excludedRoutes`) — это Go↔
/// Swift callback, который не спроектирован и не проверен: гадать про
/// него с одной стороны (Go, без реального NE) смысла нет. Поэтому здесь
/// — только статическая часть (сервер + приватные диапазоны + RU-geoip),
/// которая не имеет этой проблемы.
///
/// Используется только `engine::macos::spawn_client` — на Linux этот код
/// не компилируется (`#[cfg(target_os = "macos")]`, не `#[allow
/// (dead_code)]`: это не временно неподключённый код, а архитектурно
/// платформо-специфичная концепция, у Linux-пути своего эквивалента нет).
#[cfg(target_os = "macos")]
pub struct ExcludedRoutes {
    pub ipv4: Vec<String>,
    pub ipv6: Vec<String>,
}

#[cfg(target_os = "macos")]
pub fn generate_excluded_routes(
    app: &AppHandle,
    server: &Server,
    ru_bypass: bool,
) -> Result<ExcludedRoutes, String> {
    let (server_ipv4, server_ipv6) = resolve_server_addresses(&server.host);

    let mut ipv4: Vec<String> = vec![
        "192.168.0.0/16".into(),
        "10.0.0.0/8".into(),
        "172.16.0.0/12".into(),
        "127.0.0.0/8".into(),
    ];
    ipv4.extend(server_ipv4.iter().map(|ip| format!("{ip}/32")));

    let mut ipv6: Vec<String> = vec!["fc00::/7".into(), "fe80::/10".into()];
    ipv6.extend(server_ipv6.iter().map(|ip| format!("{ip}/128")));

    if ru_bypass {
        let (ru_ipv4, ru_ipv6) = geoip::get_ru_cidrs(app)?;
        ipv4.extend(ru_ipv4);
        ipv6.extend(ru_ipv6);
    }

    Ok(ExcludedRoutes { ipv4, ipv6 })
}

/// JSON-вариант конфига для будущего NE-пути — те же поля, что
/// `netunnel.Config` (packaging/hysteria2-patch/netunnel/netunnel.go)
/// ожидает в своём JSON, и то же подмножество, что строит `generate_config`
/// выше для YAML, БЕЗ `tun.route`/`directDomains` (это теперь
/// `generate_excluded_routes`, не часть конфига самого hysteria2-клиента)
/// и БЕЗ `quic` (в `netunnel.Config` его пока нет — см. doc-комментарий
/// в netunnel.go про time.Duration-поля, которые не парсятся так же
/// просто из JSON, как из YAML через mapstructure).
///
/// Не пишет на диск — под NE конфиг уходит в `NETunnelProviderProtocol.
/// providerConfiguration` в памяти, не файлом.
///
/// `#[cfg(target_os = "macos")]` — см. комментарий у
/// `generate_excluded_routes` выше, та же причина.
#[cfg(target_os = "macos")]
pub fn generate_provider_config_json(server: &Server) -> serde_json::Value {
    let sni = if server.sni.is_empty() {
        server.host.clone()
    } else {
        server.sni.clone()
    };

    // Резолвим host в IP здесь, а не передаём hostname в netunnel —
    // подтверждено вживую: DNS-резолвинг ВНУТРИ песочницы расширения
    // (App Sandbox) виснет на ~30с и проваливается ("no such host"),
    // судя по всему из-за includeAllNetworks — система начинает
    // захватывать трафик расширения ещё до того, как сам тоннель
    // поднялся, и его собственный DNS-запрос не проходит. SNI остаётся
    // оригинальным hostname (нужен для TLS, не для самого socket-адреса).
    let (server_ipv4, _server_ipv6) = resolve_server_addresses(&server.host);
    let server_addr = server_ipv4
        .into_iter()
        .next()
        .unwrap_or_else(|| server.host.clone());

    serde_json::json!({
        "server": format!("{}:{}", server_addr, server.port),
        "auth": server.password,
        "sni": sni,
        "insecure": server.insecure,
        "pinSHA256": server.pin_sha256,
        "obfs": {
            "type": server.obfs,
            "salamander": { "password": server.obfs_password },
        },
        "bandwidth": { "up": "100 mbps", "down": "100 mbps" },
        "inet4Addr": "100.100.100.101/30",
        "inet6Addr": "2001::ffff:ffff:ffff:fff1/126",
        "mtu": 1500,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn test_server() -> Server {
        Server {
            name: "test".into(),
            host: "vpn.example.com".into(),
            port: 443,
            password: "secret".into(),
            sni: String::new(),
            insecure: false,
            obfs: "salamander".into(),
            obfs_password: "obfspw".into(),
            pin_sha256: "AA:BB".into(),
            quic: HashMap::new(),
            raw_uri: String::new(),
        }
    }

    /// Поля и их имена должны буква-в-букву совпадать с json-тегами
    /// `netunnel.Config` (packaging/hysteria2-patch/netunnel/netunnel.go)
    /// — несовпадение здесь не поймает ни одна из сторон по отдельности
    /// (Go и Rust компилируются и тестируются независимо).
    #[cfg(target_os = "macos")]
    #[test]
    fn provider_config_json_matches_netunnel_config_shape() {
        let server = test_server();
        let json = generate_provider_config_json(&server);

        assert_eq!(json["server"], "vpn.example.com:443");
        assert_eq!(json["auth"], "secret");
        assert_eq!(json["sni"], "vpn.example.com"); // sni пуст -> fallback на host
        assert_eq!(json["insecure"], false);
        assert_eq!(json["pinSHA256"], "AA:BB");
        assert_eq!(json["obfs"]["type"], "salamander");
        assert_eq!(json["obfs"]["salamander"]["password"], "obfspw");
        assert_eq!(json["bandwidth"]["up"], "100 mbps");
        assert_eq!(json["bandwidth"]["down"], "100 mbps");
        assert!(json["inet4Addr"].is_string());
        assert!(json["mtu"].is_number());
    }
}
