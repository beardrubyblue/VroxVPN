//! Старт тоннеля: создание/сохранение `NETunnelProviderManager`,
//! `startVPNTunnelAndReturnError`, ожидание реального результата
//! (`wait_result`) и наблюдатель за неожиданным разрывом после удачного
//! коннекта (`disconnect_watcher`).

mod disconnect_watcher;
mod wait_result;

use objc2_foundation::NSString;
use objc2_network_extension::NETunnelProviderProtocol;
use tauri::AppHandle;

use disconnect_watcher::watch_for_unexpected_disconnect;
use wait_result::wait_for_connect_result_blocking;

use super::manager::{
    build_provider_configuration, load_from_preferences_blocking, load_or_create_manager_blocking,
    nserror_to_string, save_to_preferences_blocking, PROVIDER_BUNDLE_ID,
};
use crate::config_gen::{self, ExcludedRoutes};
use crate::engine::ConnectionHandle;
use crate::subscription::Server;

/// Вся синхронная objc2-логика старта тоннеля — выполняется целиком на
/// одном blocking-потоке (см. doc-комментарий модуля `engine::macos`).
/// `app` нужен здесь только для `watch_for_unexpected_disconnect`
/// (excluded routes/provider config уже посчитаны заранее, до перехода
/// на blocking-поток — `config_gen`-функциям нужен `&AppHandle` для
/// geoip/geosite, а сам `AppHandle` передаём по значению, он `Send`).
fn spawn_client_blocking(
    app: AppHandle,
    server: &Server,
    config_json: &str,
    excluded: &ExcludedRoutes,
    inet4_addr: &str,
    mtu: u32,
) -> Result<(), String> {
    let manager = load_or_create_manager_blocking()?;

    let proto = unsafe { NETunnelProviderProtocol::new() };
    unsafe {
        proto.setProviderBundleIdentifier(Some(&NSString::from_str(PROVIDER_BUNDLE_ID)));
        proto.setServerAddress(Some(&NSString::from_str(&server.host)));
        // includeAllNetworks СОЗНАТЕЛЬНО НЕ включаем — подтверждено
        // вживую (две полные потери интернета, требующие перезагрузки
        // Mac, прежде чем поняли причину): этот флаг блокирует ВЕСЬ
        // исходящий трафик системы, включая собственное соединение
        // расширения к VPN-серверу, сразу при переходе в "Connecting" —
        // до того, как setTunnelNetworkSettings вызван или startTunnel
        // завершился успехом. netunnel сам устанавливает UDP-соединение
        // к hysteria2-серверу ВНУТРИ startTunnel — то есть собственный
        // трафик тоннеля тоже блокируется этим флагом, и тоннель никогда
        // не поднимается. Документированная проблема Apple (chicken-
        // and-egg), не баг этого кода — Apple Developer Forums thread
        // 677102, wireguard-apple mailing list. Killswitch без него
        // слабее (только includedRoutes=[default] в
        // NEPacketTunnelNetworkSettings ПОСЛЕ удачного коннекта) —
        // пересмотреть отдельно, когда relay подтверждён рабочим.
        let config_dict = build_provider_configuration(config_json, excluded, inet4_addr, mtu);
        proto.setProviderConfiguration(Some(&config_dict));
    }

    unsafe {
        manager.setProtocolConfiguration(Some(&proto));
        manager.setLocalizedDescription(Some(&NSString::from_str(&format!(
            "vrox.vpn — {}",
            server.name
        ))));
        manager.setEnabled(true);
    }

    save_to_preferences_blocking(&manager)?;
    load_from_preferences_blocking(&manager)?;

    let connection = unsafe { manager.connection() };
    unsafe { connection.startVPNTunnelAndReturnError() }.map_err(|e| nserror_to_string(&e))?;

    wait_for_connect_result_blocking(&connection)?;
    watch_for_unexpected_disconnect(app, connection);
    Ok(())
}

pub async fn spawn_client(
    app: &AppHandle,
    server: &Server,
    ru_bypass: bool,
) -> Result<(ConnectionHandle, String), String> {
    // config_gen-вызовам нужен &AppHandle (geoip/geosite) — считаем их
    // ДО перехода на blocking-поток, не внутри него.
    let excluded = config_gen::generate_excluded_routes(app, server, ru_bypass)?;
    let provider_config = config_gen::generate_provider_config_json(server);
    let config_json = serde_json::to_string(&provider_config).map_err(|e| e.to_string())?;
    let inet4_addr = provider_config["inet4Addr"]
        .as_str()
        .unwrap_or("100.100.100.101")
        .to_string();
    let mtu = provider_config["mtu"].as_u64().unwrap_or(1500) as u32;
    let server = server.clone();
    let app_for_blocking = app.clone();

    tauri::async_runtime::spawn_blocking(move || {
        spawn_client_blocking(app_for_blocking, &server, &config_json, &excluded, &inet4_addr, mtu)
    })
    .await
    .map_err(|e| e.to_string())??;

    // ConnectionHandle на macOS — `()` (см. engine.rs): нет процесса,
    // который мы сами породили, тоннель живёт в `.appex`, управляемом
    // ОС. Второй элемент кортежа (на Linux — путь к YAML-конфигу) здесь
    // не нужен — kill_client заново грузит manager из system
    // preferences по bundle ID, не по этой строке.
    Ok(((), String::new()))
}
