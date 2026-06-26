//! Остановка тоннеля — TERM/KILL по пути конфига (не по pid, см.
//! doc-комментарий модуля `engine::linux`), опрос реального завершения
//! процесса перед уборкой TUN-интерфейса.

use std::process::Command;

use tauri::AppHandle;

use super::helper::run_helper;
use super::setup::cleanup_interface;
use crate::resources;

fn process_running(app: &AppHandle, config_path: &str) -> bool {
    let Ok(helper) = resources::resolve(app, "resources/privileged_helper.sh") else {
        return false;
    };
    Command::new("pkexec")
        .arg(helper)
        .args(["is-running", config_path])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// `async` + `spawn_blocking` не потому, что здесь свой deadlock (нет
/// completion-callback'ов, как в macOS-версии) — а потому, что
/// `commands.rs::disconnect` стал общим `async fn` после фикса
/// зависания на macOS (см. doc-комментарий `engine::macos::disconnect::
/// kill_client`), и до 3 секунд блокирующего опроса (`thread::sleep` в
/// цикле ниже) внутри синхронной команды держало бы поток диспетчера
/// Tauri все эти 3с — на Linux это не дедлок, но всё равно лишнее
/// удержание потока, раз уж сигнатура меняется единообразно для обеих
/// платформ.
pub async fn kill_client(app: &AppHandle, config_path: &str) -> Result<(), String> {
    let app = app.clone();
    let config_path = config_path.to_string();
    tauri::async_runtime::spawn_blocking(move || kill_client_blocking(&app, &config_path))
        .await
        .map_err(|e| e.to_string())?
}

fn kill_client_blocking(app: &AppHandle, config_path: &str) -> Result<(), String> {
    run_helper(app, &["kill-hysteria", "TERM", config_path])?;

    // ждём фактического завершения процесса опросом, а не гадаем по
    // фиксированной паузе — pkexec - обёртка, реальный root-процесс
    // vroxcore не наш child, поэтому wait() недоступен, только опрос
    // через helper. Без этого риск выдрать интерфейс из-под процесса,
    // который ещё не успел выйти.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    while std::time::Instant::now() < deadline {
        if !process_running(app, config_path) {
            cleanup_interface(app);
            return Ok(());
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
    }

    // не ответил на TERM за 3с — добиваем SIGKILL
    let _ = run_helper(app, &["kill-hysteria", "KILL", config_path]);
    std::thread::sleep(std::time::Duration::from_millis(200));
    cleanup_interface(app);
    Ok(())
}
