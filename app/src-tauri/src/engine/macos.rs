//! macOS-реализация — управление VPN через NetworkExtension.
//!
//! ЗАМЕНА прежнего sidecar+osascript+pf-подхода (удалён целиком, не
//! оставлен "на всякий случай" рядом — см. git log этого файла, если
//! нужно вернуться к нему). Тот путь реально заработал на живом Mac, но
//! был архитектурно временным мостиком: NetworkExtension всё равно
//! обязателен для iOS (см. docs/ARCHITECTURE.md), и решает на macOS то,
//! что sidecar-путь решить не мог (TCC, повторные пароли на каждый
//! привилегированный вызов, неподтверждённый killswitch через pf).
//! Поддерживать два параллельных macOS-бэкенда ради "подстраховки" не
//! имеет смысла — отсюда и решение убрать старый сразу, а не откладывать
//! до момента, когда новый заработает.
//!
//! Что принципиально меньше под NE, чем было (и чем есть на Linux):
//!   - нет sidecar-процесса вообще — hysteria2-логика встроена в
//!     `.appex`-расширение через `packaging/hysteria2-patch/netunnel/`
//!     (gomobile bind в .xcframework), процессом которого управляет ОС,
//!     а не мы;
//!   - нет привилегированного helper-скрипта и эскалации — пользователь
//!     один раз подтверждает системный VPN-профиль (как при установке
//!     любого NEVPNManager-тоннеля), дальше ОС сама управляет правами;
//!   - нет отдельного killswitch-механизма — это настройка самого
//!     тоннеля (`NEPacketTunnelNetworkSettings.includeAllNetworks =
//!     true`), а не отдельный pf-ruleset, который нужно поднимать/
//!     откатывать руками.
//!
//! ⚠ НЕ РЕАЛИЗОВАНО — это заглушка с финальной сигнатурой публичного
//! API (см. engine.rs). Управление NEVPNManager/NETunnelProviderManager
//! из Rust делается через Objective-C runtime (крейт `objc2` + биндинги
//! NetworkExtension.framework) — писать и проверять это с реальным
//! macOS SDK можно только на Mac, поэтому здесь только контракт, без
//! попытки угадать детали ObjC-вызовов вслепую. Параллельно на Mac
//! ведётся Xcode-таргет самого `.appex` (Swift, хост для netunnel).

use tauri::AppHandle;

use crate::engine::ConnectionHandle;

const NOT_IMPLEMENTED: &str =
    "macOS: подключение через NetworkExtension ещё не реализовано (см. docs/ARCHITECTURE.md)";

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

pub async fn spawn_client(_app: &AppHandle, _config_path: &str) -> Result<ConnectionHandle, String> {
    Err(NOT_IMPLEMENTED.into())
}

pub fn kill_client(_app: &AppHandle, _config_path: &str) -> Result<(), String> {
    Err(NOT_IMPLEMENTED.into())
}

/// Killswitch на NE-пути — не отдельная операция (см. doc-комментарий
/// модуля): включается как часть `NEPacketTunnelNetworkSettings` при
/// старте самого тоннеля. Здесь no-op, а не заглушка с ошибкой —
/// engine::enable_killswitch вызывается best-effort уже ПОСЛЕ удачного
/// connect (см. commands.rs), так что до реализации он не должен мешать:
/// до этой точки и так не доходит без рабочего spawn_client.
pub fn enable_killswitch(_app: &AppHandle, _vpn_server_host: &str) -> Result<(), String> {
    Ok(())
}

pub fn disable_killswitch(_app: &AppHandle) {}
