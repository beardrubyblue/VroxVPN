//! Счётчики трафика и памяти — `Tunnel.GetStats` через `VpnPlugin.stats`.
//! Заодно замечает, что тоннель пропал без нашего disconnect (ОС отозвала
//! VPN — другое VPN-приложение, «Отключить» в уведомлении): на iOS это
//! делает NEVPNStatus-наблюдатель, здесь — опрос, который фронтенд и так
//! крутит раз в секунду, пока подключено.

use tauri::{AppHandle, Emitter, Manager};

use super::plugin;
use crate::engine::{EngineState, MemoryDebug, Slot};

/// Размер страницы в /proc/self/statm — 4 КиБ на всех ABI Android.
const PAGE_SIZE: u64 = 4096;

/// RSS всего процесса: тоннель живёт в процессе приложения, отдельного
/// `.appex`, как на iOS, нет.
fn process_rss_bytes() -> u64 {
    std::fs::read_to_string("/proc/self/statm")
        .ok()
        .and_then(|statm| statm.split_whitespace().nth(1)?.parse::<u64>().ok())
        .map_or(0, |pages| pages * PAGE_SIZE)
}

fn mark_disconnected(app: &AppHandle) {
    let state = app.state::<EngineState>();
    let mut guard = state.0.lock().unwrap();
    if matches!(&*guard, Slot::Connected(_)) {
        *guard = Slot::Idle;
        drop(guard);
        let _ = app.emit("vpn-disconnected-unexpectedly", "disconnected");
    }
}

pub async fn get_traffic_totals(
    app: &AppHandle,
    _config_path: Option<&str>,
) -> Result<(u64, u64, u64, Option<MemoryDebug>), String> {
    let Some(json) = plugin::stats(app).await?.stats else {
        mark_disconnected(app);
        return Err("тоннель не запущен".into());
    };
    let parsed: serde_json::Value = serde_json::from_str(&json).map_err(|e| format!("stats: bad json: {e}"))?;
    let debug = MemoryDebug {
        heap_in_use: parsed["heapInUse"].as_u64().unwrap_or(0),
        heap_sys: parsed["heapSys"].as_u64().unwrap_or(0),
        goroutines: parsed["goroutines"].as_u64().unwrap_or(0),
        tcp_relays: parsed["tcpRelays"].as_u64().unwrap_or(0),
        udp_relays: parsed["udpRelays"].as_u64().unwrap_or(0),
        registry_size: parsed["registrySize"].as_u64().unwrap_or(0),
        avail_mem: 0,
        handshake_age_sec: parsed["handshakeAgeSec"].as_i64(),
        wg_error: parsed["wgError"].as_str().filter(|error| !error.is_empty()).map(String::from),
    };
    Ok((
        parsed["txBytes"].as_u64().unwrap_or(0),
        parsed["rxBytes"].as_u64().unwrap_or(0),
        process_rss_bytes(),
        Some(debug),
    ))
}
