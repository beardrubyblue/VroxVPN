//! Состояние демона для `vroxgw status` — JSON-файл в /run, без IPC.

use serde::{Deserialize, Serialize};

use super::paths::{RUN_DIR, STATE_FILE};

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Connecting,
    Connected,
    Error,
}

#[derive(Serialize, Deserialize)]
pub struct State {
    pub status: Status,
    pub node: Option<String>,
    pub latency_ms: Option<u32>,
    /// Unix-время последней смены статуса.
    pub since: u64,
    pub error: Option<String>,
}

impl State {
    pub fn new(status: Status, node: Option<String>, error: Option<String>) -> Self {
        let since = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|elapsed| elapsed.as_secs())
            .unwrap_or_default();
        Self { status, node, latency_ms: None, since, error }
    }

    /// Ошибка записи не роняет шлюз — `status` просто покажет старое.
    pub fn save(&self) {
        let _ = std::fs::create_dir_all(RUN_DIR);
        if let Ok(json) = serde_json::to_string_pretty(self) {
            if let Err(e) = std::fs::write(STATE_FILE, json) {
                eprintln!("[vroxgw] не удалось записать {STATE_FILE}: {e}");
            }
        }
    }

    pub fn load() -> Result<Self, String> {
        let text = std::fs::read_to_string(STATE_FILE)
            .map_err(|_| "демон не запущен (нет /run/vroxgw/state.json)".to_string())?;
        serde_json::from_str(&text).map_err(|e| e.to_string())
    }
}
