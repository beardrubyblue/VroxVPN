//! Общие типы состояния соединения + диспетчер платформ. Сама логика
//! привилегированных операций (pkexec/polkit/nftables на Linux,
//! osascript/pf на macOS) живёт в engine::linux / engine::macos —
//! см. их доккомменты. Публичный API (`ensure_polkit_rule`,
//! `loosen_rp_filter`, `cleanup_interface`, `cleanup_orphans`,
//! `spawn_client`, `kill_client`, `enable_killswitch`,
//! `disable_killswitch`) одинаковый на всех платформах, чтобы
//! commands.rs/lib.rs не знали, на какой платформе они работают.

use std::sync::Mutex;

/// Платформенно-специфичный "хвост" активного соединения, который нужно
/// освободить при disconnect, но который commands.rs не интерпретирует
/// сам (просто `drop`-ает) — на Linux это обёртка pkexec-процесса
/// (`CommandChild`), на macOS под NetworkExtension отдельного процесса,
/// который мы сами породили, не существует вообще (тоннель живёт в
/// `.appex`-расширении, управляемом ОС), поэтому там это `()`.
#[cfg(target_os = "linux")]
pub type ConnectionHandle = tauri_plugin_shell::process::CommandChild;
#[cfg(target_os = "macos")]
pub type ConnectionHandle = ();

pub struct ActiveConnection {
    pub handle: ConnectionHandle,
    pub config_path: String,
    pub server_name: String,
}

/// `Connecting`/`Disconnecting` — промежуточные состояния, которые
/// occupying-блокируют слот на время асинхронной работы (spawn/kill),
/// не отпуская Mutex между проверкой и записью — иначе два почти
/// одновременных вызова connect (например, клик в окне + событие из
/// трея) оба проходят проверку "не подключено" и оба запускают процесс.
#[derive(Default)]
pub enum Slot {
    #[default]
    Idle,
    Connecting,
    Connected(ActiveConnection),
    Disconnecting,
}

#[derive(Default)]
pub struct EngineState(pub Mutex<Slot>);

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub use linux::*;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::*;

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
compile_error!("engine.rs: поддерживаются только Linux и macOS — см. docs/MACOS_PORT.md");
