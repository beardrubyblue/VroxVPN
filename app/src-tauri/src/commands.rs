//! Tauri-команды, вызываемые фронтендом через invoke().

use serde::Serialize;
use tauri::{AppHandle, State};
use tauri_plugin_shell::ShellExt;

use crate::engine::{self, ActiveConnection, EngineState};

#[derive(Serialize, Clone)]
pub struct ConnectionStatus {
    pub connected: bool,
    pub server_name: Option<String>,
}

#[tauri::command]
pub fn get_status(state: State<EngineState>) -> ConnectionStatus {
    let guard = state.0.lock().unwrap();
    ConnectionStatus {
        connected: guard.is_some(),
        server_name: guard.as_ref().map(|c| c.config_path.clone()),
    }
}

#[tauri::command]
pub async fn connect(
    app: AppHandle,
    state: State<'_, EngineState>,
    config_path: String,
) -> Result<(), String> {
    {
        let guard = state.0.lock().unwrap();
        if guard.is_some() {
            return Err("уже подключено".into());
        }
    }

    engine::loosen_rp_filter()?;
    engine::cleanup_interface();
    let child = engine::spawn_client(&app, &config_path).await?;

    let mut guard = state.0.lock().unwrap();
    *guard = Some(ActiveConnection { child, config_path });
    Ok(())
}

#[tauri::command]
pub fn disconnect(state: State<EngineState>) -> Result<(), String> {
    let conn = {
        let mut guard = state.0.lock().unwrap();
        guard.take().ok_or("не подключено")?
    };
    engine::kill_client(&conn.config_path)?;
    // дочерний pkexec-процесс — это обёртка, не настоящий root-процесс
    // hysteria2 (см. engine.rs); реальный процесс уже убит выше
    drop(conn.child);
    Ok(())
}

/// Проверочная команда: дёргает vroxcore-sidecar без привилегий и без
/// TUN, просто чтобы убедиться, что Rust находит и запускает бинарник.
#[tauri::command]
pub async fn engine_version(app: AppHandle) -> Result<String, String> {
    let sidecar = app.shell().sidecar("vroxcore").map_err(|e| e.to_string())?;
    let output = sidecar
        .args(["version"])
        .output()
        .await
        .map_err(|e| e.to_string())?;
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}
