//! Linux: историю пишет само приложение. Тоннель — sidecar `vroxcore`,
//! а приложение живёт в трее всё время подключения, поэтому счётчики
//! `tun-vroxory` (engine::linux::get_traffic_totals) опрашиваем отсюда,
//! а не с фронтенда — тот опрашивает, только пока открыт экран.

use std::sync::Mutex;
use std::time::Duration;

use tauri::{AppHandle, Manager};

use super::{add_day, history_path, DayTraffic};
use crate::engine::{self, EngineState, Slot};

const FLUSH_INTERVAL: Duration = Duration::from_secs(5);

/// Сколько байт текущей сессии уже записано в историю. Счётчики
/// интерфейса обнуляются при каждом подключении (интерфейс создаётся
/// заново) — поэтому `start` сбрасывает и это значение.
static SAVED: Mutex<(u64, u64)> = Mutex::new((0, 0));

/// Запустить запись после успешного подключения; цикл сам завершается,
/// когда соединение перестаёт быть активным.
pub fn start(app: &AppHandle) {
    *SAVED.lock().unwrap() = (0, 0);
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(FLUSH_INTERVAL).await;
            if !is_connected(&app) {
                break;
            }
            flush(&app).await;
        }
    });
}

/// Записать накопленную дельту сейчас. Вызывается и из `disconnect` до
/// остановки клиента — пока интерфейс жив, иначе хвост сессии потерялся бы.
pub async fn flush(app: &AppHandle) {
    let Ok((upload, download, _, _)) = engine::get_traffic_totals(app, None).await else {
        return;
    };
    // Вычисление и запись — под одной блокировкой: цикл и disconnect могут
    // сбрасывать одновременно, и без неё одна дельта записалась бы дважды.
    let mut saved = SAVED.lock().unwrap();
    if upload < saved.0 || download < saved.1 {
        return; // устаревшее чтение, более свежее уже записано
    }
    let (delta_up, delta_down) = (upload - saved.0, download - saved.1);
    if delta_up == 0 && delta_down == 0 {
        return;
    }
    let date = chrono::Local::now().format("%Y-%m-%d").to_string();
    if append(app, &date, delta_up, delta_down).is_ok() {
        *saved = (upload, download);
    }
}

fn is_connected(app: &AppHandle) -> bool {
    matches!(*app.state::<EngineState>().0.lock().unwrap(), Slot::Connected(_))
}

/// Запись через временный файл + rename — экран может читать файл в любой момент.
fn append(app: &AppHandle, date: &str, upload: u64, download: u64) -> Result<(), String> {
    let path = history_path(app).ok_or("нет пути для истории трафика")?;
    let mut days: Vec<DayTraffic> = std::fs::read(&path)
        .ok()
        .and_then(|data| serde_json::from_slice(&data).ok())
        .unwrap_or_default();
    add_day(&mut days, date, upload, download);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_vec(&days).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &path).map_err(|e| e.to_string())
}
