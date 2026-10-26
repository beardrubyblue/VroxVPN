//! Протокол WireGuard / AmneziaWG. Типы узла и разбор .conf — общие с
//! шлюзом, живут в vrox-core; здесь только конфиг для NE-расширения
//! (iOS/Android), которого у шлюза нет.

#[cfg(any(target_os = "macos", target_os = "ios", target_os = "android"))]
pub mod provider;

pub use vrox_core::wireguard::*;
