//! Генерация конфига hysteria2 для TUN-режима — порт core/config_gen.py
//! (ветка main) на Rust. Здесь общая часть клиента (Linux) и шлюза:
//! YAML целиком и исключения из тоннеля. Куда писать файл и NE-варианты
//! (JSON, excludedRoutes) решает приложение.

mod yaml;

use std::net::{SocketAddr, ToSocketAddrs};

use crate::subscription::Server;

pub use yaml::generate_yaml;

/// Имя TUN-интерфейса hysteria2 — его же ждут nftables-правила шлюза.
pub const TUN_NAME: &str = "tun-vroxory";

/// Списки RU-обхода (`bypass::load_*`), уже прочитанные с диска.
#[derive(Default)]
pub struct RuBypass {
    pub ipv4: Vec<String>,
    pub ipv6: Vec<String>,
    pub domains: Vec<String>,
}

/// Приватные диапазоны и IP самого сервера — то, что не идёт в тоннель
/// никогда, независимо от RU-обхода. Для шлюза это ещё и его LAN: ответы
/// клиентам уходят напрямую, а не в тоннель.
pub fn base_excluded_routes(server: &Server) -> (Vec<String>, Vec<String>) {
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

    (ipv4, ipv6)
}

/// Резолвит host в IPv4/IPv6 адреса — нужны для exclude-маршрутов, иначе
/// пакеты к самому VPN-серверу уйдут в TUN и получится routing loop.
pub fn resolve_server_addresses(host: &str) -> (Vec<String>, Vec<String>) {
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
