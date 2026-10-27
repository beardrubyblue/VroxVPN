//! NetworkExtension (iOS) и Android VpnService: конфиг не файлом, а JSON
//! в памяти + исключённые маршруты, которые выставляет нативная сторона.

use tauri::AppHandle;
use vrox_core::bypass::load_ru_cidrs;
use vrox_core::config_gen::{base_excluded_routes, resolve_server_addresses};
use vrox_core::subscription::Server;

use crate::bypass;

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
/// Используется только `engine::macos` / `engine::android::spawn_client` — на Linux этот код
/// не компилируется (`#[cfg(any(target_os = "macos", target_os = "ios", target_os = "android"))]`, не `#[allow
/// (dead_code)]`: это не временно неподключённый код, а архитектурно
/// платформо-специфичная концепция, у Linux-пути своего эквивалента нет).
pub struct ExcludedRoutes {
    pub ipv4: Vec<String>,
    pub ipv6: Vec<String>,
}

pub fn generate_excluded_routes(
    app: &AppHandle,
    server: &Server,
    ru_bypass: bool,
) -> Result<ExcludedRoutes, String> {
    let (mut ipv4, mut ipv6) = base_excluded_routes(server);

    if ru_bypass {
        let (ru_ipv4, ru_ipv6) = load_ru_cidrs(&bypass::dirs(app)?);
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
/// `#[cfg(any(target_os = "macos", target_os = "ios", target_os = "android"))]` — см. комментарий у
/// `generate_excluded_routes` выше, та же причина.
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

    // bandwidth НЕ задаём (как и в YAML, см. vrox-core config_gen/yaml.rs):
    // заявленный Rx переводит сервер hysteria2 на Brutal с потолком ровно
    // в эту цифру — тут стояло 25 mbps на iOS и 100 на macOS, и скорость
    // упиралась в ~60-70 Мбит/с против 250-300 у Happ на той же подписке.
    // Исходно 25 mbps ставили против jetsam-килла расширения (~50МБ, см.
    // коммит 194efda), но память на приём ограничивают не bandwidth, а
    // QUIC receive windows (netunnel/quic_flow_control.go) — сервер не может
    // держать в полёте больше окна при любом congestion control.
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
            wireguard: None,
            manual_id: None,
        }
    }

    /// Поля и их имена должны буква-в-букву совпадать с json-тегами
    /// `netunnel.Config` (packaging/hysteria2-patch/netunnel/netunnel.go)
    /// — несовпадение здесь не поймает ни одна из сторон по отдельности
    /// (Go и Rust компилируются и тестируются независимо).
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
        assert!(json.get("bandwidth").is_none());
        assert!(json["inet4Addr"].is_string());
        assert!(json["mtu"].is_number());
    }
}
