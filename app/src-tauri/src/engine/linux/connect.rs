//! Старт тоннеля — `vroxcore` sidecar-процесс через `pkexec`.

use std::path::PathBuf;

use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_shell::process::CommandEvent;
use tauri_plugin_shell::ShellExt;

use crate::config_gen;
use crate::engine::{ConnectionHandle, EngineState, Slot};
use crate::subscription::Server;

const SIDECAR_NAME: &str = "vroxcore-x86_64-unknown-linux-gnu";

/// Путь к бинарнику vroxcore. Это sidecar (`bundle.externalBin`), а не
/// обычный ресурс — у Tauri для sidecar-бинарников своя конвенция
/// размещения (рядом с главным исполняемым файлом в собранном
/// приложении), а `ShellExt::sidecar()` сам его сразу запускает и не
/// отдаёт путь — а нам путь нужен отдельно, чтобы передать его в pkexec.
/// Поэтому резолвим вручную по той же конвенции: сначала рядом с
/// текущим exe (собранное приложение — Tauri при бандлинге убирает
/// суффикс target-triple из имени, проверено на реальном .deb: лежит
/// просто как "vroxcore"), иначе — src-tauri/binaries/ (`tauri dev`,
/// бинарник не "собран", лежит с суффиксом рядом с исходниками).
fn sidecar_binary_path() -> Result<PathBuf, String> {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            for name in ["vroxcore", SIDECAR_NAME] {
                let candidate = dir.join(name);
                if candidate.exists() {
                    return Ok(candidate);
                }
            }
        }
    }
    let dev_candidate =
        PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/binaries")).join(SIDECAR_NAME);
    if dev_candidate.exists() {
        return Ok(dev_candidate);
    }
    Err("бинарник vroxcore не найден ни рядом с исполняемым файлом, ни в src-tauri/binaries/".into())
}

/// `config_path`-параметр был здесь до того, как появился рабочий NE-путь
/// на macOS (см. docs/ARCHITECTURE.md, раздел "Открытый вопрос... " —
/// теперь закрыт): генерация YAML-конфига раньше жила в `commands.rs`,
/// что было утечкой sidecar-специфичной абстракции в общий API
/// (`engine::spawn_client`). Теперь генерация — здесь, внутри
/// Linux-реализации; macOS вместо файла строит JSON в памяти (см.
/// `engine/macos::connect::spawn_client`). Поведение для Linux не
/// изменилось — просто та же генерация переехала на один уровень глубже.
pub async fn spawn_client(
    app: &AppHandle,
    server: &Server,
    ru_bypass: bool,
) -> Result<(ConnectionHandle, String), String> {
    let config_path = config_gen::generate_config(app, server, ru_bypass)?;
    let config_path = config_path.to_string_lossy().to_string();

    let binary = sidecar_binary_path()?;
    let binary = binary.to_string_lossy().to_string();

    let (mut rx, child) = app
        .shell()
        .command("pkexec")
        .args([binary.as_str(), "client", "--config", config_path.as_str()])
        .spawn()
        .map_err(|e| e.to_string())?;

    let exited_early = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let exited_early_writer = exited_early.clone();
    let app_for_task = app.clone();
    tauri::async_runtime::spawn(async move {
        while let Some(event) = rx.recv().await {
            match event {
                CommandEvent::Stdout(line) => {
                    println!("[vroxcore] {}", String::from_utf8_lossy(&line));
                }
                CommandEvent::Stderr(line) => {
                    println!("[vroxcore:err] {}", String::from_utf8_lossy(&line));
                }
                CommandEvent::Terminated(payload) => {
                    println!("[vroxcore] процесс завершён: {:?}", payload.code);
                    exited_early_writer.store(true, std::sync::atomic::Ordering::SeqCst);
                    // процесс мог умереть уже ПОСЛЕ того, как connect
                    // одобрил подключение (например, сервер уронил
                    // сессию через какое-то время) — если к этому
                    // моменту state всё ещё Connected (а не Idle/
                    // Disconnecting, как было бы при обычном
                    // disconnect-вызове), отражаем разрыв в state и
                    // фронтенде, а не оставляем UI лгать про "подключено"
                    let state = app_for_task.state::<EngineState>();
                    let mut guard = state.0.lock().unwrap();
                    if matches!(&*guard, Slot::Connected(_)) {
                        *guard = Slot::Idle;
                        drop(guard);
                        let _ = app_for_task.emit("vpn-disconnected-unexpectedly", payload.code);
                    }
                }
                _ => {}
            }
        }
    });

    // короткое окно после спавна: сам факт запуска процесса (pkexec
    // успешно отработал) — не то же самое, что реально установленное
    // соединение. Если QUIC-хендшейк с сервером не проходит, vroxcore
    // завершается почти сразу с ненулевым кодом — без этой проверки
    // connect считал бы это успехом, и UI показывал бы "подключено" при
    // мёртвом тоннеле.
    tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
    if exited_early.load(std::sync::atomic::Ordering::SeqCst) {
        return Err(
            "vroxcore завершился сразу после запуска — соединение не установлено (см. лог)"
                .into(),
        );
    }

    Ok((child, config_path))
}
