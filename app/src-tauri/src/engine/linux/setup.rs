//! Подготовка/уборка системного окружения: polkit-правило, rp_filter,
//! осиротевшие процессы/TUN-интерфейс с прошлого запуска.

use std::path::Path;

use tauri::AppHandle;

use super::helper::run_helper;
use super::TUN_IFACE;

const POLKIT_RULE_PATH: &str = "/etc/polkit-1/rules.d/49-vrox-vpn-tauri.rules";

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
