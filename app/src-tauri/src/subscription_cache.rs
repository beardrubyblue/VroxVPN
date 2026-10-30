//! Кеш узлов подписок: последний удачный ответ каждой подписки.
//!
//! Почему: в settings.json хранились только `{url, name}` подписок, сами
//! узлы не сохранялись нигде — при каждом запуске экран Nodes был пустым,
//! пока фоновый `fetch_subscription` не вернётся (а без сети — пустым
//! навсегда, к последнему выбранному узлу нельзя было подключиться).
//! Теперь `fetch_subscription` после удачной загрузки пишет результат
//! сюда, фронтенд при старте сразу показывает кеш и тихо подменяет его
//! свежим ответом; ошибка загрузки кеш не трогает. Тот же подход, что у
//! шлюза (`vroxgw/src/daemon/nodes.rs`, `/var/lib/vroxgw/servers.json`).
//! Файл с паролями узлов лежит рядом с settings.json и
//! manual_servers.json в папке данных приложения (на iOS — песочница).

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use vrox_core::subscription::{self, Server, SubscriptionData, UserInfo};

const FILE_NAME: &str = "subscription_cache.json";
const FETCH_TIMEOUT_SECS: u64 = 15;

/// refreshAll грузит подписки параллельно — без замка два «прочитал →
/// дописал → сохранил» затирали бы друг друга.
static FILE_LOCK: Mutex<()> = Mutex::new(());

#[derive(Serialize, Deserialize, Clone)]
pub struct CachedSubscription {
    pub servers: Vec<Server>,
    pub userinfo: UserInfo,
    /// unix-время в мс последней удачной загрузки — для «N M AGO»
    pub updated_at: u64,
}

type Cache = HashMap<String, CachedSubscription>;

fn cache_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(app.path().app_data_dir().map_err(|e| e.to_string())?.join(FILE_NAME))
}

fn load(app: &AppHandle) -> Cache {
    cache_path(app)
        .ok()
        .and_then(|path| std::fs::read(path).ok())
        .and_then(|data| serde_json::from_slice(&data).ok())
        .unwrap_or_default()
}

fn save(app: &AppHandle, cache: &Cache) -> Result<(), String> {
    let path = cache_path(app)?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_vec(cache).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &path).map_err(|e| e.to_string())
}

fn update(app: &AppHandle, change: impl FnOnce(&mut Cache)) {
    let _guard = FILE_LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut cache = load(app);
    change(&mut cache);
    // кеш — не источник истины: не записался, значит при следующем
    // запуске узлы появятся после загрузки, как раньше
    if let Err(e) = save(app, &cache) {
        eprintln!("[subscription_cache] не удалось сохранить: {e}");
    }
}

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

#[tauri::command]
pub async fn fetch_subscription(app: AppHandle, url: String) -> Result<SubscriptionData, String> {
    let (servers, userinfo) = subscription::fetch_subscription(&url, FETCH_TIMEOUT_SECS).await?;
    let entry = CachedSubscription { servers: servers.clone(), userinfo: userinfo.clone(), updated_at: now_ms() };
    update(&app, |cache| {
        cache.insert(url, entry);
    });
    Ok(SubscriptionData { servers, userinfo })
}

/// Весь кеш: url подписки → последний удачный ответ.
#[tauri::command]
pub fn get_cached_subscriptions(app: AppHandle) -> Cache {
    let _guard = FILE_LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    load(&app)
}

/// Подписку удалили — её узлы (с паролями) на диске больше не нужны.
#[tauri::command]
pub fn forget_subscription(app: AppHandle, url: String) {
    update(&app, |cache| {
        cache.remove(&url);
    });
}
