//! NAT для клиентов LAN — своя таблица nftables `ip vroxgw`, чужие
//! правила (ufw, docker) не трогаем.
//!
//! Маскируем ВЕСЬ уходящий из LAN трафик, не только тот, что идёт в
//! тоннель: мимо тоннеля (RU-обход) пакет клиента уходит обратно на
//! MikroTik, и без NAT ответ пришёл бы клиенту напрямую от роутера, минуя
//! шлюз — асимметричный маршрут, на котором conntrack роутера помечает
//! соединения invalid.

use std::io::Write;
use std::net::Ipv4Addr;
use std::process::{Command, Stdio};

const TABLE: &str = "vroxgw";

/// `lan` подставляется в текст правил nft — проверяем, что это ровно
/// IPv4-CIDR, а не произвольная строка из конфига.
fn validate_cidr(lan: &str) -> Result<(), String> {
    let error = || format!("lan = \"{lan}\" — ожидается IPv4-подсеть вида 192.168.1.0/24");
    let (address, prefix) = lan.split_once('/').ok_or_else(error)?;
    address.parse::<Ipv4Addr>().map_err(|_| error())?;
    match prefix.parse::<u8>() {
        Ok(bits) if bits <= 32 => Ok(()),
        _ => Err(error()),
    }
}

fn run_nft(ruleset: &str) -> Result<(), String> {
    let mut child = Command::new("nft")
        .args(["-f", "-"])
        .stdin(Stdio::piped())
        .spawn()
        .map_err(|e| format!("не удалось запустить nft: {e}"))?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(ruleset.as_bytes()).map_err(|e| e.to_string())?;
    }
    let status = child.wait().map_err(|e| e.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("nft завершился с кодом {:?}", status.code()))
    }
}

/// Пересоздаёт таблицу целиком (`add` + `delete` — идемпотентно, даже
/// если её ещё нет), чтобы перезапуск демона не плодил дубли правил.
pub fn install(lan: &str) -> Result<(), String> {
    validate_cidr(lan)?;
    run_nft(&format!(
        "add table ip {TABLE}\n\
         delete table ip {TABLE}\n\
         table ip {TABLE} {{\n\
           chain postrouting {{\n\
             type nat hook postrouting priority srcnat; policy accept;\n\
             ip saddr {lan} ip daddr != {lan} masquerade\n\
           }}\n\
         }}\n"
    ))
}

pub fn remove() {
    if let Err(e) = run_nft(&format!("delete table ip {TABLE}\n")) {
        eprintln!("[vroxgw] не удалось удалить таблицу nft: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::validate_cidr;

    #[test]
    fn accepts_only_ipv4_cidr() {
        assert!(validate_cidr("192.168.1.0/24").is_ok());
        assert!(validate_cidr("10.0.0.0/8").is_ok());
        assert!(validate_cidr("192.168.1.0").is_err());
        assert!(validate_cidr("192.168.1.0/33").is_err());
        assert!(validate_cidr("192.168.1.0/24; flush ruleset").is_err());
    }
}
