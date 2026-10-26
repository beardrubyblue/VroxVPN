//! Конфиг hysteria2 под платформу. Общая часть (YAML, исключения из
//! тоннеля) — в vrox-core; здесь то, что зависит от способа запуска:
//! файл для sidecar на Linux (`linux`), JSON и excludedRoutes для NE /
//! VpnService (`ne`).

#[cfg(target_os = "linux")]
mod linux;
#[cfg(any(target_os = "macos", target_os = "ios", target_os = "android"))]
mod ne;

#[cfg(target_os = "linux")]
pub use linux::generate_config;
#[cfg(any(target_os = "macos", target_os = "ios", target_os = "android"))]
pub use ne::{generate_excluded_routes, generate_provider_config_json, ExcludedRoutes};
#[cfg(any(target_os = "macos", target_os = "ios", target_os = "android"))]
pub use vrox_core::config_gen::resolve_server_addresses;
