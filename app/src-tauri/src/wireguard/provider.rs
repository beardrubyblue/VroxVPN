//! JSON-конфиг WireGuard для NE-расширения — поля `netunnel.WireGuardConfig`
//! (packaging/hysteria2-patch/netunnel/wg_config.go); менять синхронно.
//! Плюс `inet4Addr`/`mtu`, которые engine/macos/connect читает для
//! сетевых настроек тоннеля (Go эти поля игнорирует).

use super::WireGuardProfile;
use crate::config_gen::resolve_server_addresses;

/// MTU, если в конфиге его нет (wg-easy не пишет): 1280, как официальный
/// WireGuard для iOS — с десктопными 1420 тоннель на iPhone «подключался»,
/// но крупные пакеты терялись на путях с меньшим MTU и сети не было (см.
/// netunnel/wg_config.go::defaultWireGuardMTU, менять синхронно).
const DEFAULT_MTU: u32 = 1280;

pub fn provider_config_json(profile: &WireGuardProfile) -> Result<serde_json::Value, String> {
    // Endpoint — только IP: имя резолвим здесь, внутри песочницы
    // расширения DNS до поднятия тоннеля не работает (config_gen.rs,
    // generate_provider_config_json — та же причина для hysteria2).
    let (host, port) = profile.endpoint_host_port()?;
    let (ipv4, ipv6) = resolve_server_addresses(&host);
    let endpoint = match (ipv4.first(), ipv6.first()) {
        (Some(ip), _) => format!("{ip}:{port}"),
        (None, Some(ip)) => format!("[{ip}]:{port}"),
        (None, None) => return Err(format!("Не удалось найти адрес сервера {host}")),
    };
    let inet4 = profile
        .addresses
        .iter()
        .find(|address| address.contains('.'))
        .ok_or("В конфиге нет IPv4-адреса")?;
    let mtu = profile.mtu.unwrap_or(DEFAULT_MTU);

    Ok(serde_json::json!({
        "protocol": "wireguard",
        "privateKey": profile.private_key,
        "mtu": mtu,
        "obfuscation": profile.obfuscation,
        "peer": {
            "publicKey": profile.peer.public_key,
            "presharedKey": profile.peer.preshared_key,
            "endpoint": endpoint,
            "allowedIPs": profile.peer.allowed_ips,
            "persistentKeepalive": profile.peer.persistent_keepalive,
        },
        "inet4Addr": inet4,
    }))
}

#[cfg(test)]
mod tests {
    use crate::wireguard::parse_conf;

    /// Имена полей — буква в букву json-теги netunnel.WireGuardConfig
    /// (wg_config.go): расхождение не поймает ни Go, ни Rust по отдельности.
    #[test]
    fn provider_json_matches_netunnel_wireguard_config() {
        let conf = "[Interface]\nPrivateKey = cHJpdg==\nAddress = 10.8.0.14/24\nJc = 4\n\
            [Peer]\nPublicKey = cHVi\nPresharedKey = cHNr\nAllowedIPs = 0.0.0.0/0\nEndpoint = 192.0.2.55:51820\n";
        let json = super::provider_config_json(&parse_conf(conf).unwrap()).unwrap();
        assert_eq!(json["protocol"], "wireguard");
        assert_eq!(json["privateKey"], "cHJpdg==");
        assert_eq!(json["mtu"], 1280);
        assert_eq!(json["obfuscation"]["jc"], "4");
        assert_eq!(json["peer"]["publicKey"], "cHVi");
        assert_eq!(json["peer"]["presharedKey"], "cHNr");
        assert_eq!(json["peer"]["endpoint"], "192.0.2.55:51820");
        assert_eq!(json["peer"]["allowedIPs"][0], "0.0.0.0/0");
        assert_eq!(json["peer"]["persistentKeepalive"], 0);
        assert_eq!(json["inet4Addr"], "10.8.0.14/24");
    }

    /// Живая цепочка Rust → Go: разобрать настоящий .conf (VROX_WG_TEST_CONF)
    /// и записать JSON для расширения в VROX_WG_TEST_OUT — его потом
    /// поднимает netunnel/wireguard_live_test.go. Без переменных — пропуск.
    #[test]
    fn write_provider_json_for_live_test() {
        let (Ok(conf_path), Ok(out_path)) = (std::env::var("VROX_WG_TEST_CONF"), std::env::var("VROX_WG_TEST_OUT")) else {
            return;
        };
        let conf = std::fs::read_to_string(conf_path).unwrap();
        let mut json = super::provider_config_json(&parse_conf(&conf).unwrap()).unwrap();
        json["memoryConstrained"] = serde_json::Value::Bool(false);
        std::fs::write(out_path, json.to_string()).unwrap();
    }
}
