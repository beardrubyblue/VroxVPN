//! История трафика по дням (последний месяц) для экрана Stats.
//!
//! На iOS (iPhone/iPad/Mac) файл пишет само NE-расширение
//! (packaging/hysteria2-patch/netunnel/history.go) в App Group — оно
//! видит каждый байт, даже когда приложение закрыто, поэтому цифры
//! точные. Приложение здесь только читает. На Linux тоннель — sidecar, а
//! приложение живёт в трее всё время подключения, поэтому пишет сам Rust
//! (`recorder`). Формат файла общий — менять синхронно с history.go.

#[cfg(target_os = "linux")]
pub mod recorder;

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri::AppHandle;

/// Ровно месяц — по столбику на день; старше не храним (history.go::historyDays).
#[cfg(any(target_os = "linux", test))]
pub const HISTORY_DAYS: usize = 30;
const FILE_NAME: &str = "traffic_history.json";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct DayTraffic {
    /// Локальная дата `YYYY-MM-DD`.
    pub date: String,
    pub upload: u64,
    pub download: u64,
}

/// Дни по возрастанию даты; нет файла (ещё ни одного подключения) или он
/// битый — пустой список, экран покажет пустое состояние.
pub fn read(app: &AppHandle) -> Vec<DayTraffic> {
    history_path(app)
        .and_then(|path| std::fs::read(path).ok())
        .and_then(|data| serde_json::from_slice(&data).ok())
        .unwrap_or_default()
}

/// Прибавить трафик к дню, сохранить порядок по дате и последние
/// HISTORY_DAYS дней — та же логика, что history.go::addDayTraffic.
/// На iOS пишет Go, поэтому здесь нужно только Linux-записи (и тесту).
#[cfg(any(target_os = "linux", test))]
pub fn add_day(days: &mut Vec<DayTraffic>, date: &str, upload: u64, download: u64) {
    match days.iter_mut().find(|day| day.date == date) {
        Some(day) => {
            day.upload += upload;
            day.download += download;
        }
        None => days.push(DayTraffic { date: date.to_string(), upload, download }),
    }
    days.sort_by(|first, second| first.date.cmp(&second.date));
    let excess = days.len().saturating_sub(HISTORY_DAYS);
    days.drain(..excess);
}

/// App Group, общая с расширением (entitlements обоих таргетов,
/// PacketTunnelProvider.swift::appGroupID).
#[cfg(any(target_os = "macos", target_os = "ios"))]
const APP_GROUP_ID: &str = "group.com.vroxory.vpn";

#[cfg(any(target_os = "macos", target_os = "ios"))]
fn history_path(_app: &AppHandle) -> Option<PathBuf> {
    use objc2_foundation::{NSFileManager, NSString};
    let group = NSString::from_str(APP_GROUP_ID);
    let url = NSFileManager::defaultManager().containerURLForSecurityApplicationGroupIdentifier(&group)?;
    Some(PathBuf::from(url.path()?.to_string()).join(FILE_NAME))
}

#[cfg(target_os = "linux")]
fn history_path(app: &AppHandle) -> Option<PathBuf> {
    use tauri::Manager;
    app.path().app_data_dir().ok().map(|dir| dir.join(FILE_NAME))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_day_accumulates_and_keeps_last_month_sorted() {
        let mut days = Vec::new();
        add_day(&mut days, "2026-09-30", 100, 1000);
        add_day(&mut days, "2026-09-30", 5, 50);
        assert_eq!(days, vec![DayTraffic { date: "2026-09-30".into(), upload: 105, download: 1050 }]);

        for day in (1..=31).rev() {
            add_day(&mut days, &format!("2026-08-{day:02}"), 1, 1);
        }
        assert_eq!(days.len(), HISTORY_DAYS);
        assert_eq!(days.last().unwrap().date, "2026-09-30");
        assert!(days.windows(2).all(|pair| pair[0].date < pair[1].date));
    }
}
