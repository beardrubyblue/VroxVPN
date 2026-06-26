//! Общая инфраструктура вызова `privileged_helper.sh` через `pkexec` —
//! три варианта (по коду возврата / захват stdout / передача stdin),
//! используются всеми остальными подмодулями `engine::linux`.

use std::io::Write;
use std::process::{Command, Stdio};

use tauri::AppHandle;

use crate::resources;

pub(super) fn run_helper(app: &AppHandle, args: &[&str]) -> Result<(), String> {
    let helper = resources::resolve(app, "resources/privileged_helper.sh")?;
    let status = Command::new("pkexec")
        .arg(helper)
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

/// Как `run_helper`, но возвращает stdout — нужен только для `mem-usage`
/// (остальные команды helper'а возвращают результат через exit code).
pub(super) fn run_helper_capture(app: &AppHandle, args: &[&str]) -> Result<String, String> {
    let helper = resources::resolve(app, "resources/privileged_helper.sh")?;
    let output = Command::new("pkexec")
        .arg(helper)
        .args(args)
        .output()
        .map_err(|e| e.to_string())?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        Err(format!(
            "privileged_helper.sh {:?} завершился с кодом {:?}: {}",
            args,
            output.status.code(),
            String::from_utf8_lossy(&output.stderr)
        ))
    }
}

pub(super) fn run_helper_with_stdin(app: &AppHandle, args: &[&str], input: &str) -> Result<(), String> {
    let helper = resources::resolve(app, "resources/privileged_helper.sh")?;
    let mut child = Command::new("pkexec")
        .arg(helper)
        .args(args)
        .stdin(Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;
    child
        .stdin
        .take()
        .ok_or("не удалось открыть stdin pkexec")?
        .write_all(input.as_bytes())
        .map_err(|e| e.to_string())?;
    let status = child.wait().map_err(|e| e.to_string())?;
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
