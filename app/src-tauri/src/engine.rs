//! Общие типы состояния соединения + диспетчер платформ. Сама логика
//! привилегированных операций (pkexec/polkit/nftables на Linux,
//! NetworkExtension на macOS/iOS — API NETunnelProviderManager у них
//! общий, поэтому это один и тот же модуль `engine::macos`, не два)
//! живёт в engine::linux / engine::macos — см. их доккомменты.
//! Публичный API (`ensure_polkit_rule`, `loosen_rp_filter`,
//! `cleanup_interface`, `cleanup_orphans`, `spawn_client`,
//! `kill_client`, `enable_killswitch`, `disable_killswitch`)
//! одинаковый на всех платформах, чтобы commands.rs/lib.rs не знали,
//! на какой платформе они работают.

use std::sync::Mutex;

use serde::Serialize;

/// Детальная разбивка памяти тоннеля — только macOS/iOS, где весь стек
/// (Go-рантайм + gVisor + QUIC) живёт внутри одного `.appex`-процесса с
/// жёстким лимитом ~50МБ. Нужна, чтобы видеть в UI КУДА именно уходит
/// память при росте под нагрузкой (Reels): Go-куча vs RSS всего процесса
/// vs число активных relay/горутин. Linux оставляет `None` — там тоннель
/// это внешний `vroxcore`, эти внутренние счётчики недоступны (и не
/// нужны: нет жёсткого лимита процесса). Поля зеркалят JSON от
/// `netunnel.go::TunnelHandle.GetStats` (см.).
#[derive(Serialize, Clone, Default)]
pub struct MemoryDebug {
    /// Go heap, реально занятый живыми объектами (runtime.MemStats.HeapInuse).
    pub heap_in_use: u64,
    /// Вся память, которую Go-рантайм запросил у ОС (runtime.MemStats.Sys).
    pub heap_sys: u64,
    /// Число живых горутин — растёт, если relay-горутины утекают.
    pub goroutines: u64,
    /// Активные TCP-relay (handler.go::activeTCPRelays).
    pub tcp_relays: u64,
    /// Активные UDP-relay (handler.go::activeUDPRelays).
    pub udp_relays: u64,
    /// Размер реестра соединений (handler.go::connRegistry) — должен
    /// совпадать с tcp+udp relays; расхождение = утечка регистрации.
    pub registry_size: u64,
    /// Сколько байт ОС ещё готова дать процессу (os_proc_available_memory,
    /// только iOS) — на macOS вернётся 0/огромное.
    pub avail_mem: u64,
}

/// Платформенно-специфичный "хвост" активного соединения, который нужно
/// освободить при disconnect, но который commands.rs не интерпретирует
/// сам (просто `drop`-ает) — на Linux это обёртка pkexec-процесса
/// (`CommandChild`), на macOS/iOS под NetworkExtension отдельного
/// процесса, который мы сами породили, не существует вообще (тоннель
/// живёт в `.appex`-расширении, управляемом ОС), поэтому там это `()`.
#[cfg(target_os = "linux")]
pub type ConnectionHandle = tauri_plugin_shell::process::CommandChild;
#[cfg(any(target_os = "macos", target_os = "ios"))]
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

#[cfg(any(target_os = "macos", target_os = "ios"))]
mod macos;
#[cfg(any(target_os = "macos", target_os = "ios"))]
pub use macos::*;

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "ios")))]
compile_error!("engine.rs: поддерживаются только Linux, macOS и iOS — см. docs/MACOS_PORT.md");
