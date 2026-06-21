//! Пинг серверов — порт core/ping.py. hysteria2 работает по UDP/QUIC,
//! поэтому TCP-connect к порту сервера обычно не проходит даже при живом
//! сервере — используем системный `ping` (ICMP), как и в Python-версии.

use serde::Serialize;
use tokio::process::Command;

async fn ping_host(host: &str, timeout_secs: u64) -> Option<u32> {
    let wait = timeout_secs.max(1).to_string();
    let output = Command::new("ping")
        .args(["-c", "1", "-W", &wait, host])
        .output()
        .await
        .ok()?;
    if !output.status.success() {
        return None;
    }
    parse_time_ms(&String::from_utf8_lossy(&output.stdout))
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
}

pub async fn ping_all(servers: Vec<(String, String)>) -> Vec<PingResult> {
    let futures = servers.into_iter().map(|(name, host)| async move {
        let latency_ms = ping_host(&host, 3).await;
        PingResult { name, latency_ms }
    });
    futures::future::join_all(futures).await
}
