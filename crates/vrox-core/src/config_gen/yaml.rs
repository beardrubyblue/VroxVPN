//! YAML-конфиг hysteria2-клиента (наш форк `vroxcore`) для TUN-режима.

use serde_yaml::{Mapping, Value};

use super::{base_excluded_routes, RuBypass, TUN_NAME};
use crate::subscription::Server;

fn s(v: impl Into<String>) -> Value {
    Value::String(v.into())
}

fn str_seq(items: impl IntoIterator<Item = String>) -> Value {
    Value::Sequence(items.into_iter().map(Value::String).collect())
}

/// Текст YAML для сервера. `ru_bypass`: geoip-исключения IP-диапазонов
/// России в маршрутах TUN и directDomains (geosite) для сайтов на
/// зарубежном CDN, которые под geoip не попадают — см. docs/ARCHITECTURE.md.
pub fn generate_yaml(server: &Server, ru_bypass: Option<RuBypass>) -> Result<String, String> {
    let (mut ipv4_exclude, mut ipv6_exclude) = base_excluded_routes(server);
    let mut direct_domains: Vec<String> = Vec::new();
    if let Some(bypass) = ru_bypass {
        ipv4_exclude.extend(bypass.ipv4);
        ipv6_exclude.extend(bypass.ipv6);
        direct_domains = bypass.domains;
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
    tun.insert(s("name"), s(TUN_NAME));
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

    // bandwidth НЕ задаём намеренно. Раньше тут стояло up/down 100 mbps —
    // и это резало скорость до ~60-70 Мбит/с, тогда как Happ на той же
    // подписке давал 250-300. Причина в сервере hysteria2
    // (core/server/server.go, обработка handshake): если клиент заявил
    // свой Rx, сервер вместо BBR включает Brutal и шлёт РОВНО с этой
    // скоростью — заявленная цифра становится потолком загрузки. Без
    // bandwidth сервер берёт свой congestion control (BBR), как у Happ.

    let mut config = Mapping::new();
    config.insert(s("server"), s(format!("{}:{}", server.host, server.port)));
    config.insert(s("auth"), s(server.password.clone()));
    config.insert(s("tls"), Value::Mapping(tls));
    config.insert(s("tun"), Value::Mapping(tun));
    config.insert(s("fastOpen"), Value::Bool(true));

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

    serde_yaml::to_string(&Value::Mapping(config)).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_server() -> Server {
        Server {
            name: "test".into(),
            host: "127.0.0.1".into(),
            port: 443,
            password: "secret".into(),
            obfs: "salamander".into(),
            obfs_password: "obfspw".into(),
            ..Default::default()
        }
    }

    #[test]
    fn yaml_contains_tun_excludes_and_direct_domains() {
        let bypass = RuBypass {
            ipv4: vec!["5.0.0.0/8".into()],
            ipv6: vec![],
            domains: vec!["ya.ru".into()],
        };
        let yaml = generate_yaml(&test_server(), Some(bypass)).unwrap();
        let config: Value = serde_yaml::from_str(&yaml).unwrap();

        assert_eq!(config["server"], s("127.0.0.1:443"));
        assert_eq!(config["tun"]["name"], s(TUN_NAME));
        assert_eq!(config["tls"]["sni"], s("127.0.0.1")); // sni пуст -> host
        assert_eq!(config["obfs"]["salamander"]["password"], s("obfspw"));
        let excludes = config["tun"]["route"]["ipv4Exclude"].as_sequence().unwrap();
        assert!(excludes.contains(&s("192.168.0.0/16")));
        assert!(excludes.contains(&s("127.0.0.1/32")));
        assert!(excludes.contains(&s("5.0.0.0/8")));
        assert_eq!(config["tun"]["directDomains"][0], s("ya.ru"));
    }

    #[test]
    fn yaml_without_bypass_has_no_direct_domains() {
        let yaml = generate_yaml(&test_server(), None).unwrap();
        let config: Value = serde_yaml::from_str(&yaml).unwrap();
        assert!(config["tun"].get("directDomains").is_none());
    }

    #[test]
    fn yaml_has_no_bandwidth() {
        // Заявленный bandwidth переводит сервер на Brutal с потолком
        // скорости — см. комментарий в generate_yaml.
        let yaml = generate_yaml(&test_server(), None).unwrap();
        let config: Value = serde_yaml::from_str(&yaml).unwrap();
        assert!(config.get("bandwidth").is_none());
    }
}
