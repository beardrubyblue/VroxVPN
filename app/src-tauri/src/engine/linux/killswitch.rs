//! Kill switch — nftables-таблица с `policy drop` на output, пропускающая
//! только TUN/loopback/локальные сети/сам VPN-сервер.

use std::net::{IpAddr, SocketAddr, ToSocketAddrs};

use tauri::AppHandle;

use super::helper::{run_helper, run_helper_with_stdin};
use super::TUN_IFACE;

const KILLSWITCH_TABLE: &str = "vroxory_killswitch";

/// Резолвит host сервера в один IPv4-литерал для kill switch-правила —
/// порт core/kill_switch.py::_safe_server_ip. Непроверенная строка из
/// подписки никогда не должна попадать прямо в текст nft-правил,
/// выполняемых от root, поэтому при неудаче резолва возвращаем
/// заведомо нерабочий адрес, а не исходную строку.
fn safe_server_ip(host: &str) -> String {
    if let Ok(ip) = host.parse::<IpAddr>() {
        if ip.is_ipv4() {
            return ip.to_string();
        }
    }
    if let Ok(addrs) = format!("{host}:0").to_socket_addrs() {
        for addr in addrs {
            if let SocketAddr::V4(a) = addr {
                return a.ip().to_string();
            }
        }
    }
    "0.0.0.0/32".to_string()
}

/// Включает kill switch — nftables-таблица с `policy drop` на output,
/// пропускающая только TUN/loopback/локальные сети/сам VPN-сервер.
/// Порт core/kill_switch.py::KillSwitch.enable. Опциональная защита
/// (тогл в настройках, выключена по умолчанию) — в Python-версии была
/// спрятана из UI из-за нестабильности, здесь включается аккуратно:
/// best-effort cleanup при старте приложения и при disconnect (см.
/// disable_killswitch), чтобы неудачное отключение не блокировало сеть
/// навсегда.
pub fn enable_killswitch(app: &AppHandle, vpn_server_host: &str) -> Result<(), String> {
    let ip = safe_server_ip(vpn_server_host);
    let rules = format!(
        "table inet {KILLSWITCH_TABLE} {{
    chain output {{
        type filter hook output priority 0; policy drop;
        oifname \"lo\" accept
        oifname \"{TUN_IFACE}\" accept
        ip daddr {ip} accept
        ip daddr 192.168.0.0/16 accept
        ip daddr 10.0.0.0/8 accept
        ip daddr 172.16.0.0/12 accept
    }}
}}
"
    );
    run_helper_with_stdin(app, &["nft-apply"], &rules)
}

/// Снимает kill switch — best-effort: если таблицы нет (kill switch не
/// был включён или уже снят), nft вернёт ошибку, которую игнорируем.
pub fn disable_killswitch(app: &AppHandle) {
    let _ = run_helper(app, &["nft-delete-table", KILLSWITCH_TABLE]);
}
