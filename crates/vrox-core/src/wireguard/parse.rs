//! Разбор конфига wg-quick (.conf), как его выдают wg-easy и AmneziaVPN.

use super::{WireGuardPeer, WireGuardProfile};

/// Параметры маскировки AmneziaWG — в .conf с заглавной (Jc, S1, H1…).
const OBFUSCATION_KEYS: &[&str] = &[
    "jc", "jmin", "jmax", "s1", "s2", "s3", "s4", "h1", "h2", "h3", "h4", "i1", "i2", "i3", "i4", "i5",
];

enum Section {
    None,
    Interface,
    Peer,
}

fn split_list(value: &str) -> Vec<String> {
    value.split(',').map(str::trim).filter(|item| !item.is_empty()).map(String::from).collect()
}

/// DNS из конфига не
/// используется: DNS тоннеля одинаковый для всех протоколов (DoH в
/// PacketTunnelProvider.swift).
pub fn parse_conf(text: &str) -> Result<WireGuardProfile, String> {
    let mut profile = WireGuardProfile::default();
    let mut section = Section::None;
    let mut peers = 0;

    for raw_line in text.lines() {
        let line = raw_line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        match line.to_ascii_lowercase().as_str() {
            "[interface]" => {
                section = Section::Interface;
                continue;
            }
            "[peer]" => {
                peers += 1;
                section = Section::Peer;
                continue;
            }
            _ => {}
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let (key, value) = (key.trim().to_ascii_lowercase(), value.trim().to_string());
        match section {
            Section::Interface => match key.as_str() {
                "privatekey" => profile.private_key = value,
                "address" => profile.addresses.extend(split_list(&value)),
                "mtu" => profile.mtu = value.parse().ok(),
                key if OBFUSCATION_KEYS.contains(&key) => {
                    profile.obfuscation.insert(key.to_string(), value);
                }
                _ => {} // DNS, ListenPort, PostUp… — клиенту не нужны
            },
            Section::Peer => apply_peer_key(&mut profile.peer, &key, value),
            Section::None => {}
        }
    }

    if peers != 1 {
        return Err("В конфиге должен быть ровно один [Peer]".into());
    }
    validate(&profile)?;
    Ok(profile)
}

fn apply_peer_key(peer: &mut WireGuardPeer, key: &str, value: String) {
    match key {
        "publickey" => peer.public_key = value,
        "presharedkey" => peer.preshared_key = value,
        "endpoint" => peer.endpoint = value,
        "allowedips" => peer.allowed_ips.extend(split_list(&value)),
        "persistentkeepalive" => peer.persistent_keepalive = value.parse().unwrap_or(0),
        _ => {}
    }
}

fn validate(profile: &WireGuardProfile) -> Result<(), String> {
    if profile.private_key.is_empty() {
        return Err("В [Interface] нет PrivateKey".into());
    }
    if !profile.addresses.iter().any(|address| address.contains('.')) {
        return Err("В [Interface] нет IPv4-адреса (Address)".into());
    }
    if profile.peer.public_key.is_empty() {
        return Err("В [Peer] нет PublicKey".into());
    }
    if profile.peer.allowed_ips.is_empty() {
        return Err("В [Peer] нет AllowedIPs".into());
    }
    profile.endpoint_host_port().map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    const WG_EASY: &str = "[Interface]\nPrivateKey = cHJpdg==\nAddress = 10.8.0.14/24\nDNS = 1.1.1.1\n\n\
        [Peer]\nPublicKey = cHVi\nPresharedKey = cHNr\nAllowedIPs = 0.0.0.0/0, ::/0\n\
        PersistentKeepalive = 0\nEndpoint = 195.0.2.55:51820\n";

    #[test]
    fn parses_wg_easy_conf() {
        let profile = parse_conf(WG_EASY).unwrap();
        assert_eq!(profile.addresses, vec!["10.8.0.14/24"]);
        assert_eq!(profile.peer.allowed_ips, vec!["0.0.0.0/0", "::/0"]);
        assert_eq!(profile.endpoint_host_port().unwrap(), ("195.0.2.55".into(), 51820));
        assert!(profile.obfuscation.is_empty());
    }

    #[test]
    fn parses_amneziawg_params_and_rejects_bad_conf() {
        let awg = WG_EASY.replace("DNS = 1.1.1.1", "Jc = 4\nS1 = 15\nH1 = 1234");
        let profile = parse_conf(&awg).unwrap();
        assert_eq!(profile.obfuscation.get("jc").map(String::as_str), Some("4"));
        assert_eq!(profile.obfuscation.len(), 3);
        assert!(parse_conf("[Interface]\nPrivateKey = x\n").is_err());
    }
}
