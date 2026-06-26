//! Пинг серверов — порт core/ping.py. hysteria2 работает по UDP/QUIC,
//! поэтому TCP-connect к порту сервера обычно не проходит даже при живом
//! сервере — используем системный `ping` (ICMP), как и в Python-версии.

use serde::Serialize;
use tokio::process::Command;

/// `Err` — диагностика для UI (показывается по тапу на прочерк, см.
/// App.tsx), не просто `None` — раньше любая причина провала (хост не
/// резолвится, `ping` не найден, реально таймаут, permission denied)
/// выглядела в UI одинаково как голый прочерк, и было невозможно
/// отличить "сервер недоступен" от "у нас сломан вызов команды" без
/// доступа к терминалу пользователя.
async fn ping_host(host: &str, timeout_secs: u64) -> Result<u32, String> {
    let wait = timeout_secs.max(1).to_string();
    // `-W` означает разное на Linux и macOS: на Linux (iputils) — секунды
    // ожидания ответа, на macOS (BSD ping) — МИЛЛИСЕКУНДЫ (см. man ping).
    // Раньше код был портирован с Linux без учёта этого — на macOS
    // `-W 3` означало "ждать ответ всего 3мс", что гарантированно
    // проваливало пинг почти любого реального хоста (RTT обычно
    // десятки-сотни мс), отсюда прочерки в UI у всех пользователей
    // macOS. На macOS используем `-t` — общий таймаут в секундах, тот
    // же смысл, что у Linux-варианта `-W`.
    #[cfg(target_os = "macos")]
    let timeout_flag = "-t";
    #[cfg(not(target_os = "macos"))]
    let timeout_flag = "-W";
    let output = Command::new("ping")
        .args(["-c", "1", timeout_flag, &wait, host])
        .output()
        .await
        .map_err(|e| format!("не удалось запустить ping: {e}"))?;
    if !output.status.success() {
        // Реальная причина провала (100% packet loss, Request timeout,
        // Name or service not known...) у системного `ping` уходит в
        // stdout, не в stderr — stderr у него обычно пустой даже при
        // полном провале (подтверждено вживую). Берём то, что не пусто,
        // предпочитая stderr, если он всё-таки что-то содержит.
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        return Err(if !stderr.is_empty() {
            stderr
        } else if !stdout.is_empty() {
            stdout
        } else {
            format!("ping завершился с кодом {:?} без вывода", output.status.code())
        });
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_time_ms(&stdout).ok_or_else(|| format!("не удалось разобрать вывод ping: {stdout}"))
}

fn parse_time_ms(text: &str) -> Option<u32> {
    let idx = text.find("time=").or_else(|| text.find("time<"))?;
    let rest = &text[idx + 5..];
    let end = rest
        .find(|c: char| !(c.is_ascii_digit() || c == '.'))
        .unwrap_or(rest.len());
    rest[..end].parse::<f64>().ok().map(|v| v.round() as u32)
}

#[derive(Serialize)]
pub struct PingResult {
    pub name: String,
    pub latency_ms: Option<u32>,
    pub error: Option<String>,
}

pub async fn ping_all(servers: Vec<(String, String)>) -> Vec<PingResult> {
    let futures = servers.into_iter().map(|(name, host)| async move {
        match ping_host(&host, 3).await {
            Ok(ms) => PingResult { name, latency_ms: Some(ms), error: None },
            Err(e) => PingResult { name, latency_ms: None, error: Some(e) },
        }
    });
    futures::future::join_all(futures).await
}
