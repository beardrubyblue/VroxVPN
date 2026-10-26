//! Команды администратора: `status`, `nodes`, `switch`. Работают с
//! файлами демона (state.json, кеш подписки) — без IPC.

use std::process::Command;

use crate::daemon::load_cache;
use crate::settings::paths::NODE_OVERRIDE;
use crate::settings::{State, Status, NODE_AUTO};

pub const USAGE: &str = "\
vroxgw — шлюз vrox.vpn (hysteria2) для локальной сети

  vroxgw status          состояние тоннеля
  vroxgw nodes           узлы из подписки
  vroxgw switch <имя>    закрепить узел (auto — снова по пингу)
  vroxgw run             запуск демона (это делает systemd)

Конфиг: /etc/vroxgw/config.toml, лог: journalctl -u vroxgw";

fn status_label(status: Status) -> &'static str {
    match status {
        Status::Connecting => "подключается",
        Status::Connected => "подключён",
        Status::Error => "ошибка",
    }
}

pub fn status() -> Result<(), String> {
    let state = State::load()?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or_default();
    println!("Статус: {} ({} с назад)", status_label(state.status), now.saturating_sub(state.since));
    if let Some(node) = &state.node {
        let latency = state.latency_ms.map(|ms| format!(", пинг {ms} мс")).unwrap_or_default();
        println!("Узел:   {node}{latency}");
    }
    if let Some(error) = &state.error {
        println!("Ошибка: {error}");
    }
    Ok(())
}

pub fn nodes() -> Result<(), String> {
    let servers = load_cache().map_err(|_| "подписка ещё не загружалась".to_string())?;
    let current = State::load().ok().and_then(|state| state.node);
    for server in servers {
        let marker = if current.as_deref() == Some(server.name.as_str()) { "●" } else { " " };
        println!("{marker} {}", server.name);
    }
    Ok(())
}

pub fn switch(name: Option<&str>) -> Result<(), String> {
    let name = name.ok_or("укажите имя узла или auto")?;
    if name == NODE_AUTO {
        if let Err(e) = std::fs::remove_file(NODE_OVERRIDE) {
            if e.kind() != std::io::ErrorKind::NotFound {
                return Err(e.to_string());
            }
        }
    } else {
        let servers = load_cache().map_err(|_| "подписка ещё не загружалась".to_string())?;
        if !servers.iter().any(|server| server.name == name) {
            return Err(format!("узла «{name}» нет в подписке (см. vroxgw nodes)"));
        }
        std::fs::write(NODE_OVERRIDE, name).map_err(|e| format!("{NODE_OVERRIDE}: {e}"))?;
    }
    let status = Command::new("systemctl")
        .args(["restart", "vroxgw"])
        .status()
        .map_err(|e| format!("не удалось перезапустить сервис: {e}"))?;
    if !status.success() {
        return Err("systemctl restart vroxgw не удался (нужен root)".into());
    }
    println!("Узел: {name}, сервис перезапущен");
    Ok(())
}
