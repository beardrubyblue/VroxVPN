//! Протокол WireGuard / AmneziaWG: разбор конфигов wg-quick (.conf) и
//! конфиг для NE-расширения. Сам тоннель — Go (packaging/hysteria2-patch/
//! netunnel/wireguard.go). Хранение добавленных вручную узлов (любого
//! протокола) — в manual/. Только iOS: на Linux WireGuard пока не
//! поддерживается (engine/linux отказывает, manual/ не даёт добавить).
//!
//! WireGuard-узел — обычный `Server` с заполненным `wireguard`: выбор,
//! пинг, трей и исключение IP сервера из тоннеля работают как у hysteria2.

mod parse;
#[cfg(any(target_os = "macos", target_os = "ios"))]
pub mod provider;

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

pub use parse::parse_conf;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct WireGuardPeer {
    pub public_key: String,
    pub preshared_key: String,
    /// Как в .conf: `host:port` или `[ipv6]:port`.
    pub endpoint: String,
    pub allowed_ips: Vec<String>,
    pub persistent_keepalive: u16,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct WireGuardProfile {
    pub private_key: String,
    /// `Address` из .conf, CIDR (IPv4 и/или IPv6).
    pub addresses: Vec<String>,
    pub mtu: Option<u32>,
    /// Параметры маскировки AmneziaWG строчными (jc, s1, h1…); пусто —
    /// обычный WireGuard.
    pub obfuscation: BTreeMap<String, String>,
    pub peer: WireGuardPeer,
}

impl WireGuardProfile {
    /// Хост и порт сервера из `Endpoint` (IPv6 — в квадратных скобках).
    pub fn endpoint_host_port(&self) -> Result<(String, u16), String> {
        let (host, port) = self
            .peer
            .endpoint
            .rsplit_once(':')
            .ok_or("Endpoint без порта")?;
        let port = port.parse::<u16>().map_err(|_| "неверный порт в Endpoint")?;
        Ok((host.trim_start_matches('[').trim_end_matches(']').to_string(), port))
    }
}
