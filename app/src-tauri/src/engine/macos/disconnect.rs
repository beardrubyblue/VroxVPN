//! Остановка тоннеля — `stopVPNTunnel` через свежезагруженный
//! `NETunnelProviderManager`.

use tauri::AppHandle;

use super::manager::load_or_create_manager_blocking;

/// `async` + `spawn_blocking` обязателен здесь, не просто для паритета
/// сигнатур (см. doc-комментарий модуля `engine::macos` про
/// `connect::spawn_client`) — без этого был реальный deadlock,
/// подтверждённый вживую: `disconnect` был обычной синхронной
/// Tauri-командой, вызывавшей этот код прямо на потоке диспетчера
/// команд. `load_or_create_manager_blocking()` внутри блокируется на
/// `rx.recv()`, ожидая completion-callback от
/// `loadAllFromPreferencesWithCompletionHandler` — а тот callback
/// должен прийти через главный run loop приложения. Если поток
/// диспетчера команд и есть главный поток — он блокирует сам себя:
/// ждёт callback, который не может быть доставлен, потому что run loop
/// (на том же главном потоке) не крутится. Внешне это выглядело как
/// полный фриз всего приложения (не только кнопки) при попытке
/// disconnect — VPN-тоннель при этом РЕАЛЬНО отключался на уровне ОС
/// (`scutil --nc list` показывал Disconnected), просто Rust-сторона
/// никогда не получала об этом подтверждения и не возвращала ответ
/// фронтенду.
pub async fn kill_client(app: &AppHandle, config_path: &str) -> Result<(), String> {
    let _ = (app, config_path);
    tauri::async_runtime::spawn_blocking(kill_client_blocking)
        .await
        .map_err(|e| e.to_string())?
}

fn kill_client_blocking() -> Result<(), String> {
    let manager = load_or_create_manager_blocking()?;
    let connection = unsafe { manager.connection() };
    unsafe { connection.stopVPNTunnel() };
    Ok(())
}
