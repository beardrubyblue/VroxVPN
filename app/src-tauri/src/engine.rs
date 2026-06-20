//! Управление процессом vroxcore (наш форк hysteria2) привилегированно —
//! TUN-режим требует root/CAP_NET_ADMIN.
//!
//! Логика connect/disconnect зеркалит core/tun_manager.py из ветки main
//! (тот же контракт privileged_helper.sh): kill идёт по пути конфига,
//! а не по pid процесса — pkexec отдаёт супервизору pid самой обёртки
//! pkexec, а не настоящего root-процесса hysteria2, поэтому убить его
//! напрямую по pid невозможно (см. helper-скрипт, секция kill-hysteria).

use std::process::Command;
use std::sync::Mutex;

use tauri::AppHandle;
use tauri_plugin_shell::process::{CommandChild, CommandEvent};
use tauri_plugin_shell::ShellExt;

const TUN_IFACE: &str = "tun-vroxory";

// TODO(packaging): путь резолвится относительно исходников для разработки.
// В собранном приложении нужно брать его через
// app.path().resolve(_, BaseDirectory::Resource) — см. docs/ARCHITECTURE.md.
const PRIVILEGED_HELPER: &str =
    concat!(env!("CARGO_MANIFEST_DIR"), "/resources/privileged_helper.sh");
const SIDECAR_BINARY: &str =
    concat!(env!("CARGO_MANIFEST_DIR"), "/binaries/vroxcore-x86_64-unknown-linux-gnu");

pub struct ActiveConnection {
    pub child: CommandChild,
    pub config_path: String,
    pub server_name: String,
}

#[derive(Default)]
pub struct EngineState(pub Mutex<Option<ActiveConnection>>);

fn run_helper(args: &[&str]) -> Result<(), String> {
    let status = Command::new("pkexec")
        .arg(PRIVILEGED_HELPER)
        .args(args)
        .status()
        .map_err(|e| e.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "privileged_helper.sh {:?} завершился с кодом {:?}",
            args,
            status.code()
        ))
    }
}

pub fn loosen_rp_filter() -> Result<(), String> {
    // Linux отбрасывает TUN-трафик строгим reverse-path filter — без
    // этого пакеты к серверу маршрутизируются, но ответы дропаются ядром
    run_helper(&["loosen-rp-filter"])
}

pub fn cleanup_interface() {
    // best-effort, как в Python: если предыдущий запуск завершился
    // аварийно, интерфейс tun-vroxory может остаться висеть в ядре —
    // тогда hysteria2 падает с "device or resource busy"
    let _ = run_helper(&["delete-tun", TUN_IFACE]);
}

pub async fn spawn_client(app: &AppHandle, config_path: &str) -> Result<CommandChild, String> {
    let (mut rx, child) = app
        .shell()
        .command("pkexec")
        .args([SIDECAR_BINARY, "client", "--config", config_path])
        .spawn()
        .map_err(|e| e.to_string())?;

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
                }
                _ => {}
            }
        }
    });

    Ok(child)
}

pub fn kill_client(config_path: &str) -> Result<(), String> {
    run_helper(&["kill-hysteria", "TERM", config_path])?;
    cleanup_interface();
    Ok(())
}
