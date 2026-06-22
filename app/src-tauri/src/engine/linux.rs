//! Linux-реализация привилегированного слоя — pkexec/polkit/nftables.
//! Логика connect/disconnect зеркалит core/tun_manager.py из ветки main
//! (тот же контракт privileged_helper.sh): kill идёт по пути конфига,
//! а не по pid процесса — pkexec отдаёт супервизору pid самой обёртки
//! pkexec, а не настоящего root-процесса hysteria2, поэтому убить его
//! напрямую по pid невозможно (см. helper-скрипт, секция kill-hysteria).

use std::io::Write;
use std::net::{IpAddr, SocketAddr, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_shell::process::CommandEvent;
use tauri_plugin_shell::ShellExt;

use crate::config_gen;
use crate::engine::{ConnectionHandle, EngineState, Slot};
use crate::resources;
use crate::subscription::Server;

const TUN_IFACE: &str = "tun-vroxory";
const KILLSWITCH_TABLE: &str = "vroxory_killswitch";
const POLKIT_RULE_PATH: &str = "/etc/polkit-1/rules.d/49-vrox-vpn-tauri.rules";
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

fn run_helper(app: &AppHandle, args: &[&str]) -> Result<(), String> {
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

fn run_helper_with_stdin(app: &AppHandle, args: &[&str], input: &str) -> Result<(), String> {
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

/// Резолвит host сервера в один IPv4-литерал для kill switch-правила —
/// порт core/kill_switch.py::_safe_server_ip. Непроверенная строка из
/// подписки никогда не должна попадать прямо в текст nft-правил,
/// выполняемых от root, поэтому при неудаче резолва возвращаем
/// заведомо нерабочий адрес, а не исходную строку.
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

/// Включает kill switch — nftables-таблица с `policy drop` на output,
/// пропускающая только TUN/loopback/локальные сети/сам VPN-сервер.
/// Порт core/kill_switch.py::KillSwitch.enable. Опциональная защита
/// (тогл в настройках, выключена по умолчанию) — в Python-версии была
/// спрятана из UI из-за нестабильности, здесь включается аккуратно:
/// best-effort cleanup при старте приложения и при disconnect (см.
/// disable_killswitch), чтобы неудачное отключение не блокировало сеть
/// навсегда.
pub fn enable_killswitch(app: &AppHandle, vpn_server_host: &str) -> Result<(), String> {
    let ip = safe_server_ip(vpn_server_host);
    let rules = format!(
        "table inet {KILLSWITCH_TABLE} {{
    chain output {{
        type filter hook output priority 0; policy drop;
        oifname \"lo\" accept
        oifname \"{TUN_IFACE}\" accept
        ip daddr {ip} accept
        ip daddr 192.168.0.0/16 accept
        ip daddr 10.0.0.0/8 accept
        ip daddr 172.16.0.0/12 accept
    }}
}}
"
    );
    run_helper_with_stdin(app, &["nft-apply"], &rules)
}

/// Снимает kill switch — best-effort: если таблицы нет (kill switch не
/// был включён или уже снят), nft вернёт ошибку, которую игнорируем.
pub fn disable_killswitch(app: &AppHandle) {
    let _ = run_helper(app, &["nft-delete-table", KILLSWITCH_TABLE]);
}

/// Один pkexec-запрос пароля на весь жизненный цикл приложения (а не на
/// каждый отдельный privileged-вызов): пишет polkit-правило, разрешающее
/// passwordless pkexec для privileged_helper.sh и vroxcore. Если правило
/// уже стоит (не первый запуск) — не дёргает pkexec вообще.
pub fn ensure_polkit_rule(app: &AppHandle) -> Result<(), String> {
    if Path::new(POLKIT_RULE_PATH).exists() {
        return Ok(());
    }
    run_helper(app, &["install-polkit-rule"])
}

pub fn loosen_rp_filter(app: &AppHandle) -> Result<(), String> {
    // Linux отбрасывает TUN-трафик строгим reverse-path filter — без
    // этого пакеты к серверу маршрутизируются, но ответы дропаются ядром
    run_helper(app, &["loosen-rp-filter"])
}

/// Вызывается один раз при старте приложения — подчищает осиротевший
/// root-процесс vroxcore и TUN-интерфейс, если предыдущий запуск
/// приложения был убит/крашнулся до disconnect (см. kill-all-hysteria
/// в privileged_helper.sh).
pub fn cleanup_orphans(app: &AppHandle) {
    let _ = run_helper(app, &["kill-all-hysteria"]);
    cleanup_interface(app);
}

pub fn cleanup_interface(app: &AppHandle) {
    // best-effort, как в Python: если предыдущий запуск завершился
    // аварийно, интерфейс tun-vroxory может остаться висеть в ядре —
    // тогда hysteria2 падает с "device or resource busy"
    let _ = run_helper(app, &["delete-tun", TUN_IFACE]);
}

/// `config_path`-параметр был здесь до того, как появился рабочий NE-путь
/// на macOS (см. docs/ARCHITECTURE.md, раздел "Открытый вопрос... " —
/// теперь закрыт): генерация YAML-конфига раньше жила в `commands.rs`,
/// что было утечкой sidecar-специфичной абстракции в общий API
/// (`engine::spawn_client`). Теперь генерация — здесь, внутри
/// Linux-реализации; macOS вместо файла строит JSON в памяти (см.
/// `engine/macos.rs::spawn_client`). Поведение для Linux не изменилось —
/// просто та же генерация переехала на один уровень глубже.
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

pub fn kill_client(app: &AppHandle, config_path: &str) -> Result<(), String> {
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
