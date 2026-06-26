//! macOS-реализация — управление VPN через NetworkExtension.
//!
//! ЗАМЕНА прежнего sidecar+osascript+pf-подхода (удалён целиком, не
//! оставлен "на всякий случай" рядом — см. git log этого файла, если
//! нужно вернуться к нему). Тот путь реально заработал на живом Mac, но
//! был архитектурно временным мостиком: NetworkExtension всё равно
//! обязателен для iOS (см. docs/ARCHITECTURE.md), и решает на macOS то,
//! что sidecar-путь решить не мог (TCC, повторные пароли на каждый
//! привилегированный вызов, неподтверждённый killswitch через pf).
//!
//! Control-bridge к NEVPNManager/NETunnelProviderManager — крейт
//! `objc2-network-extension` (готовый, генерируемый из заголовков
//! NetworkExtension.framework, не пришлось писать ручные
//! `extern_class!`/`msg_send!` биндинги, как предполагалось в
//! ARCHITECTURE.md на момент написания плана).
//!
//! Важная деталь реализации: `Retained<NETunnelProviderManager>` и
//! `block2::RcBlock` НЕ `Send` — а Tauri требует `Send`-future от async
//! команд (`#[tauri::command] async fn connect`, см. commands.rs).
//! Поэтому вся объективно-цишная логика здесь синхронна (блокирующие
//! `std::sync::mpsc`-каналы вместо `tokio::sync::oneshot`/`.await`) и
//! выполняется целиком на одном выделенном потоке через
//! `tauri::async_runtime::spawn_blocking` — снаружи у `spawn_client`/
//! `kill_client` остаётся обычная асинхронная сигнатура (для паритета с
//! Linux-реализацией), но внутри await пересекает только `JoinHandle`,
//! результат которого (`Result<(), String>` и т.п.) — `Send`. Completion-
//! блоки NE вызываются на главном потоке (run loop приложения), фоновый
//! поток просто блокируется на `recv()`, ожидая результата — главный
//! поток в это время свободен крутить свой run loop как обычно.
//!
//! Сам `.appex` (NEPacketTunnelProvider, хост для `netunnel` через
//! gomobile) — отдельный Xcode-проект `macos-ext/` (Swift, не Rust) —
//! см. `docs/ARCHITECTURE.md`, раздел "Фаза 2".
//!
//! Разбито на подмодули по фазе жизненного цикла соединения:
//! `manager` (NETunnelProviderManager — общая инфраструктура),
//! `connect` (старт тоннеля), `disconnect` (остановка), `stats`
//! (счётчики трафика/памяти).

mod connect;
mod disconnect;
mod manager;
mod stats;

pub use connect::spawn_client;
pub use disconnect::kill_client;
pub use stats::get_traffic_totals;

use tauri::AppHandle;

/// На Linux здесь пишется polkit-правило на весь жизненный цикл
/// приложения. NE не нуждается в отдельном шаге авторизации заранее —
/// разрешение даётся один раз при установке VPN-профиля через системный
/// диалог, не через privileged-helper.
pub fn ensure_polkit_rule(_app: &AppHandle) -> Result<(), String> {
    Ok(())
}

/// rp_filter — Linux-специфичный sysctl, аналога на macOS нет.
pub fn loosen_rp_filter(_app: &AppHandle) -> Result<(), String> {
    Ok(())
}

/// utun-интерфейс полностью под управлением ОС/NE-расширения — нет
/// отдельного шага "удалить интерфейс", как на Linux.
pub fn cleanup_interface(_app: &AppHandle) {}

/// Нет sidecar-процесса, который мог бы осиротеть при крахе приложения.
pub fn cleanup_orphans(_app: &AppHandle) {}

/// Killswitch на NE-пути — не отдельная операция: обеспечивается
/// `includedRoutes = [NEIPv4Route.default()]` в `NEPacketTunnelNetworkSettings`
/// (`PacketTunnelProvider.swift::startTunnel`) — весь трафик и так идёт
/// через тоннель, как только он реально поднят. `includeAllNetworks`
/// СОЗНАТЕЛЬНО НЕ используется (убран, см. `connect::spawn_client_blocking`
/// и `docs/ARCHITECTURE.md`) — он блокирует и собственный трафик
/// расширения до того, как тоннель поднялся. Здесь no-op, а не заглушка
/// с ошибкой — `engine::enable_killswitch` вызывается best-effort уже
/// ПОСЛЕ удачного connect (см. commands.rs), так что он не должен мешать
/// или дублировать работу.
pub fn enable_killswitch(_app: &AppHandle, _vpn_server_host: &str) -> Result<(), String> {
    Ok(())
}

pub fn disable_killswitch(_app: &AppHandle) {}
