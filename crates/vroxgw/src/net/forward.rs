//! IPv4-форвардинг ядра — им же реализован kill switch: пока тоннель не
//! поднят, ядро просто не пересылает пакеты клиентов LAN, и они не могут
//! уйти в интернет напрямую мимо VPN. Отдельные drop-правила для этого не
//! нужны: с RU-обходом часть трафика клиентов законно идёт мимо тоннеля
//! (исключённые маршруты и directDomains), и правило «только в tun»
//! ломало бы обход.
//!
//! IPv6 шлюз не форвардит: клиентам LAN он выдаётся только как IPv4-шлюз.

const IP_FORWARD: &str = "/proc/sys/net/ipv4/ip_forward";

pub fn set_forwarding(is_enabled: bool) -> Result<(), String> {
    let value = if is_enabled { "1" } else { "0" };
    std::fs::write(IP_FORWARD, value).map_err(|e| format!("не удалось записать {IP_FORWARD}: {e}"))
}
