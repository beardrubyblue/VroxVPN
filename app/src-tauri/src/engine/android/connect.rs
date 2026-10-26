//! Старт и остановка тоннеля через `VpnPlugin` (Kotlin).

use tauri::AppHandle;

use super::plugin::{self, StartArgs};
use crate::config_gen;
use crate::engine::ConnectionHandle;
use crate::subscription::Server;

pub async fn spawn_client(
    app: &AppHandle,
    server: &Server,
    ru_bypass: bool,
) -> Result<(ConnectionHandle, String), String> {
    let mut excluded = config_gen::generate_excluded_routes(app, server, ru_bypass)?;
    if let Some(profile) = &server.wireguard {
        crate::wireguard::provider::keep_tunnel_subnet(&mut excluded, profile);
    }
    // протокол узла: WireGuard — импортированный .conf, иначе hysteria2
    let mut config = match &server.wireguard {
        Some(profile) => crate::wireguard::provider::provider_config_json(profile)?,
        None => config_gen::generate_provider_config_json(server),
    };
    // То, что на iOS добавляет расширение (PacketTunnelProvider.withEnvironment):
    // жёсткого лимита памяти, как у NE (~50 МБ), у обычного процесса
    // Android нет; историю пишет Go в данные приложения — оттуда же её
    // читает traffic_history.
    config["memoryConstrained"] = false.into();
    if let Some(path) = crate::traffic_history::history_path(app) {
        config["historyPath"] = path.to_string_lossy().into_owned().into();
    }

    let args = StartArgs {
        inet4_addr: config["inet4Addr"].as_str().unwrap_or("100.100.100.101/30").to_string(),
        inet6_addr: config["inet6Addr"].as_str().map(String::from),
        mtu: config["mtu"].as_u64().unwrap_or(1500) as u32,
        config_json: config.to_string(),
        ipv4_exclude: excluded.ipv4,
        ipv6_exclude: excluded.ipv6,
        session_name: server.name.clone(),
    };
    plugin::start(app, args).await?;

    // ConnectionHandle — `()`: тоннелем владеет VroxVpnService.
    Ok(((), String::new()))
}

pub async fn kill_client(app: &AppHandle, _config_path: &str) -> Result<(), String> {
    plugin::stop(app).await
}
