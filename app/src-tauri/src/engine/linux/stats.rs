//! Счётчики трафика (`/proc/net/dev`) и памяти (`/proc/<pid>/status`
//! root-процесса `vroxcore` через privileged helper).

use tauri::AppHandle;

use super::helper::run_helper_capture;
use super::TUN_IFACE;

/// (upload_bytes, download_bytes) — суммарно с начала тоннеля, читается
/// прямо с `tun-vroxory` (rx/tx ядра, не наши собственные счётчики — в
/// отличие от macOS, здесь реальный TUN-интерфейс существует). Тот же
/// файл и те же поля, что и в `core/stats.py::_read_interface_bytes` из
/// ветки main (см. docs/ARCHITECTURE.md): `/proc/net/dev`, поле 0 — rx
/// bytes (входящее = download), поле 8 — tx bytes (исходящее = upload).
/// Дельту/скорость считает фронтенд между двумя опросами, не здесь.
pub async fn get_traffic_totals(
    app: &AppHandle,
    config_path: Option<&str>,
) -> Result<(u64, u64, u64), String> {
    let (upload_bytes, download_bytes) = read_interface_bytes(TUN_IFACE)?;
    let memory_bytes = match config_path {
        Some(path) => {
            let app = app.clone();
            let path = path.to_string();
            tauri::async_runtime::spawn_blocking(move || mem_usage_blocking(&app, &path))
                .await
                .map_err(|e| e.to_string())??
        }
        // не подключено — нет процесса, который можно было бы спросить
        None => 0,
    };
    Ok((upload_bytes, download_bytes, memory_bytes))
}

fn mem_usage_blocking(app: &AppHandle, config_path: &str) -> Result<u64, String> {
    run_helper_capture(app, &["mem-usage", config_path])?
        .parse::<u64>()
        .map_err(|e| format!("mem-usage: bad output: {e}"))
}

fn read_interface_bytes(interface: &str) -> Result<(u64, u64), String> {
    let content = std::fs::read_to_string("/proc/net/dev").map_err(|e| e.to_string())?;
    for line in content.lines().skip(2) {
        let Some((name, data)) = line.split_once(':') else {
            continue;
        };
        if name.trim() != interface {
            continue;
        }
        let fields: Vec<&str> = data.split_whitespace().collect();
        let rx_bytes: u64 = fields.first().and_then(|s| s.parse().ok()).unwrap_or(0);
        let tx_bytes: u64 = fields.get(8).and_then(|s| s.parse().ok()).unwrap_or(0);
        return Ok((tx_bytes, rx_bytes));
    }
    Err(format!(
        "интерфейс {interface} не найден в /proc/net/dev (тоннель не активен?)"
    ))
}
