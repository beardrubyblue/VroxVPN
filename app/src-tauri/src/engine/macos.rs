//! macOS-реализация привилегированного слоя.
//!
//! ⚠ НЕ ПРОВЕРЕНО НА РЕАЛЬНОМ MACOS — писалось и компилировалось только
//! на Linux (cross-check невозможен: нет std для x86_64/aarch64-apple-darwin
//! без rustup-таргета, а часть API чисто macOS-специфичная). Перед тем
//! как доверять этому в проде — пройти весь чеклист в docs/MACOS_PORT.md.
//!
//! Главное архитектурное отличие от Linux:
//!   - нет pkexec/polkit → используется `osascript ... with administrator
//!     privileges` (системный GUI-промпт). В отличие от polkit-правила,
//!     это НЕ даёт гарантии "спросить пароль один раз навсегда" — macOS
//!     кеширует авторизацию ненадолго (порядка нескольких минут), но не
//!     бессрочно. Правильный путь к паритету с Linux — SMAppService
//!     (привилегированный daemon, регистрируется один раз через System
//!     Settings) — см. план в docs/MACOS_PORT.md, здесь его пока нет.
//!   - нет nftables → используется pf (pfctl) через privileged_helper_macos.sh
//!   - TUN на macOS — это utun-интерфейс с ИМЕНЕМ, НАЗНАЧАЕМЫМ ЯДРОМ
//!     (utun0, utun1, ...), а не задаваемым нами как на Linux. Поэтому
//!     kill switch здесь блокирует исходящий трафик на физических
//!     интерфейсах (Wi-Fi/Ethernet) кроме как до самого VPN-сервера, а не
//!     "разрешает только TUN" — так не нужно знать имя utun-интерфейса
//!     заранее.

use std::net::{IpAddr, SocketAddr, ToSocketAddrs};
use std::path::PathBuf;
use std::process::Command;

use tauri::AppHandle;
use tauri_plugin_shell::process::{CommandChild, CommandEvent};
use tauri_plugin_shell::ShellExt;

use crate::resources;

const SIDECAR_NAME_X86: &str = "vroxcore-x86_64-apple-darwin";
const SIDECAR_NAME_ARM: &str = "vroxcore-aarch64-apple-darwin";

fn sidecar_binary_path() -> Result<PathBuf, String> {
    let names: &[&str] = if cfg!(target_arch = "aarch64") {
        &["vroxcore", SIDECAR_NAME_ARM]
    } else {
        &["vroxcore", SIDECAR_NAME_X86]
    };
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            for name in names {
                let candidate = dir.join(name);
                if candidate.exists() {
                    return Ok(candidate);
                }
            }
        }
    }
    for name in names {
        let dev_candidate =
            PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/binaries")).join(name);
        if dev_candidate.exists() {
            return Ok(dev_candidate);
        }
    }
    Err("бинарник vroxcore не найден ни рядом с исполняемым файлом, ни в src-tauri/binaries/".into())
}

/// Экранирует строку для вставки в одинарные кавычки POSIX shell —
/// `'` заменяется на `'\''` (закрыть кавычку, экранированная кавычка,
/// открыть кавычку снова).
fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

/// Экранирует строку для вставки в двойные кавычки AppleScript —
/// экранируем `\` и `"`.
fn applescript_quote(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Строит `osascript -e 'do shell script "..." with administrator
/// privileges'` — единственный встроенный способ запросить root без
/// собственного привилегированного daemon (SMAppService). Каждый вызов
/// потенциально показывает системный диалог с паролем/Touch ID.
fn elevated_shell_command(args: &[&str]) -> String {
    let inner = args
        .iter()
        .map(|a| shell_quote(a))
        .collect::<Vec<_>>()
        .join(" ");
    format!(
        "do shell script \"{}\" with administrator privileges",
        applescript_quote(&inner)
    )
}

/// Копирует privileged_helper_macos.sh в /tmp перед каждой эскалацией.
/// Причина — НЕ косметика: `do shell script ... with administrator
/// privileges` на macOS не может исполнить файл, лежащий в TCC-защищённой
/// папке (~/Documents, ~/Desktop, ~/Downloads, iCloud Drive) — привилегиро-
/// ванный процесс получает `Operation not permitted` независимо от прав
/// файла (проверено вручную: тот же файл из /tmp выполняется нормально,
/// `sudo` из Terminal с тем же путём в Documents — тоже нормально, так что
/// дело именно в этом конкретном механизме эскалации). /tmp TCC не защищён.
fn stage_helper_outside_tcc(original: &std::path::Path) -> Result<String, String> {
    use std::os::unix::fs::PermissionsExt;
    let dest = std::path::PathBuf::from("/tmp/vroxory-vpn-privileged-helper.sh");
    std::fs::copy(original, &dest).map_err(|e| e.to_string())?;
    std::fs::set_permissions(&dest, std::fs::Permissions::from_mode(0o755)).map_err(|e| e.to_string())?;
    Ok(dest.to_string_lossy().to_string())
}

fn run_helper(app: &AppHandle, args: &[&str]) -> Result<(), String> {
    let helper = resources::resolve(app, "resources/privileged_helper_macos.sh")?;
    let helper = stage_helper_outside_tcc(&helper)?;
    // Явно передаём скрипт как аргумент /bin/bash, а не полагаемся на его
    // шебанг: на части macOS-версий `do shell script ... with
    // administrator privileges` не может получить cwd для процесса,
    // который ядро форкнуло САМО по шебангу скрипта ("shell-init: error
    // retrieving current directory", exit 126) — а единичный прямой exec
    // /bin/bash с путём к скрипту в аргументах (без форка по шебангу)
    // этой проблемы не имеет (проверено вручную).
    let mut full_args = vec!["/bin/bash", helper.as_str()];
    full_args.extend_from_slice(args);
    let script = elevated_shell_command(&full_args);
    let status = Command::new("osascript")
        .arg("-e")
        .arg(&script)
        .status()
        .map_err(|e| e.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "privileged_helper_macos.sh {:?} завершился с кодом {:?}",
            args,
            status.code()
        ))
    }
}

/// На Linux это пишет polkit-правило (один pkexec-запрос пароля на весь
/// жизненный цикл приложения). На macOS такого механизма нет — каждый
/// `osascript ... with administrator privileges` спрашивает пароль
/// независимо (см. doc-комментарий модуля). No-op до реализации
/// SMAppService-хелпера.
pub fn ensure_polkit_rule(_app: &AppHandle) -> Result<(), String> {
    Ok(())
}

/// rp_filter — Linux-специфичный sysctl, на macOS нет аналога в этом виде.
pub fn loosen_rp_filter(_app: &AppHandle) -> Result<(), String> {
    Ok(())
}

/// utun-интерфейс на macOS привязан к файловому дескриптору процесса,
/// который его создал — закрылся процесс, ядро само убирает интерфейс.
/// В отличие от Linux TUN, здесь нет отдельного шага "удалить интерфейс".
pub fn cleanup_interface(_app: &AppHandle) {}

pub fn cleanup_orphans(app: &AppHandle) {
    let _ = run_helper(app, &["kill-all-hysteria"]);
}

pub async fn spawn_client(app: &AppHandle, config_path: &str) -> Result<CommandChild, String> {
    let binary = sidecar_binary_path()?;
    let binary = binary.to_string_lossy().to_string();

    let script = elevated_shell_command(&[binary.as_str(), "client", "--config", config_path]);

    let (mut rx, child) = app
        .shell()
        .command("osascript")
        .args(["-e", &script])
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

fn process_running(app: &AppHandle, config_path: &str) -> bool {
    run_helper(app, &["is-running", config_path]).is_ok()
}

pub fn kill_client(app: &AppHandle, config_path: &str) -> Result<(), String> {
    run_helper(app, &["kill-hysteria", "TERM", config_path])?;

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    while std::time::Instant::now() < deadline {
        if !process_running(app, config_path) {
            return Ok(());
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
    }

    let _ = run_helper(app, &["kill-hysteria", "KILL", config_path]);
    Ok(())
}

/// Резолвит host сервера в один IPv4-литерал — см. аналог в linux.rs
/// (core/kill_switch.py::_safe_server_ip). При неудаче резолва
/// возвращаем заведомо нерабочий адрес, а не исходную строку: она
/// никогда не должна попадать прямо в текст pf-правил, выполняемых от
/// root, без валидации.
fn safe_server_ip(host: &str) -> String {
    if let Ok(ip) = host.parse::<IpAddr>() {
        if ip.is_ipv4() {
            return ip.to_string();
        }
    }
    if let Ok(addrs) = format!("{host}:0").to_socket_addrs() {
        for addr in addrs {
            if let SocketAddr::V4(a) = addr {
                return a.ip().to_string();
            }
        }
    }
    "0.0.0.0/32".to_string()
}

/// ⚠ НЕ ПРОВЕРЕНО — pf-ruleset строится самим shell-скриптом
/// (privileged_helper_macos.sh, подкоманда pf-apply), а не здесь, потому
/// что enumerate физических интерфейсов (`ifconfig -l`) естественнее
/// делать в shell. Сюда передаём только resolved IP сервера.
pub fn enable_killswitch(app: &AppHandle, vpn_server_host: &str) -> Result<(), String> {
    let ip = safe_server_ip(vpn_server_host);
    run_helper(app, &["pf-apply", &ip])
}

pub fn disable_killswitch(app: &AppHandle) {
    let _ = run_helper(app, &["pf-restore"]);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_quote_escapes_single_quotes() {
        assert_eq!(shell_quote("it's"), r"'it'\''s'");
    }

    #[test]
    fn applescript_quote_escapes_quotes_and_backslashes() {
        assert_eq!(applescript_quote(r#"a"b\c"#), r#"a\"b\\c"#);
    }
}
