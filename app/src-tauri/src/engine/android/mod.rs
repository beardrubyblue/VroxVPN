//! Android-реализация — VpnService.
//!
//! Тоннель тот же Go `netunnel` (hysteria2 / WireGuard), что у iOS, только
//! собран в `.aar` (android/build-go-aar.sh) и живёт прямо в процессе
//! приложения: `VroxVpnService` (Kotlin, gen/android/app/src/main/java/
//! com/vroxory/vpn/tunnel/) получает TUN-fd от ОС и отдаёт его в
//! `netunnel.StartFD` — пакеты качает Go, без JNI на каждый пакет.
//!
//! Rust разговаривает с Kotlin через мобильный плагин Tauri (`plugin`):
//! команды start / stop / stats. Конфиг тоннеля и исключённые маршруты
//! (сервер, локальные сети, RU-обход) считаются здесь теми же функциями,
//! что у iOS (`config_gen`, `wireguard::provider`) — Kotlin только
//! передаёт их в `VpnService.Builder`.

mod connect;
mod plugin;
mod stats;

pub use connect::{kill_client, spawn_client};
pub use plugin::init as vpn_plugin;
pub use stats::get_traffic_totals;

use tauri::AppHandle;

/// Разрешение на VPN — системный диалог `VpnService.prepare` при первом
/// connect (Kotlin, `VpnPlugin.start`), не отдельный шаг заранее.
pub fn ensure_polkit_rule(_app: &AppHandle) -> Result<(), String> {
    Ok(())
}

/// rp_filter — Linux-специфичный sysctl.
pub fn loosen_rp_filter(_app: &AppHandle) -> Result<(), String> {
    Ok(())
}

/// TUN-интерфейс принадлежит VpnService — ОС убирает его сама.
pub fn cleanup_interface(_app: &AppHandle) {}

/// Нет внешнего процесса, который мог бы осиротеть.
pub fn cleanup_orphans(_app: &AppHandle) {}

/// Весь IPv4-трафик и так идёт в тоннель (маршрут 0.0.0.0/0 в
/// `VpnService.Builder`). Полный killswitch на Android — системная
/// настройка «Постоянная VPN» + «Блокировать без VPN», приложение её
/// включить не может.
pub fn enable_killswitch(_app: &AppHandle, _vpn_server_host: &str) -> Result<(), String> {
    Ok(())
}

pub fn disable_killswitch(_app: &AppHandle) {}
