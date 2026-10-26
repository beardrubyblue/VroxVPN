//! Мост к Kotlin-плагину `VpnPlugin` (gen/android/app/src/main/java/com/
//! vroxory/vpn/tunnel/VpnPlugin.kt). Kotlin-класс лежит прямо в модуле app
//! сгенерированного проекта — отдельная библиотека-плагин не нужна:
//! Tauri ищет класс через загрузчик классов activity.

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use tauri::plugin::{Builder, PluginHandle, TauriPlugin};
use tauri::{AppHandle, Manager, Wry};

const PLUGIN_NAME: &str = "vpn";
const KOTLIN_PACKAGE: &str = "com.vroxory.vpn.tunnel";
const KOTLIN_CLASS: &str = "VpnPlugin";

struct VpnPluginHandle(PluginHandle<Wry>);

/// Аргументы `VpnPlugin.start` — имена полей совпадают с `StartArgs` в Kotlin.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartArgs {
    /// JSON для `netunnel.StartFD` (тот же формат, что у iOS).
    pub config_json: String,
    /// Адрес интерфейса, CIDR (`100.100.100.101/30` или адрес из .conf).
    pub inet4_addr: String,
    /// IPv6-адрес интерфейса, CIDR; есть только у hysteria2. С ним весь
    /// IPv6 тоже идёт в тоннель — без маршрута ::/0 Android пустил бы
    /// IPv6 приложений мимо VPN. У WireGuard `None` — там IPv4, как на iOS.
    pub inet6_addr: Option<String>,
    pub mtu: u32,
    /// Маршруты мимо тоннеля — `VpnService.Builder.excludeRoute` (API 33+).
    pub ipv4_exclude: Vec<String>,
    pub ipv6_exclude: Vec<String>,
    /// Имя узла — в постоянное уведомление.
    pub session_name: String,
}

/// Ответ `VpnPlugin.stats`: `stats` — JSON от `Tunnel.GetStats`, `None`,
/// если тоннель не запущен (например, ОС отозвала VPN).
#[derive(Deserialize)]
pub struct StatsResponse {
    pub stats: Option<String>,
}

#[derive(Deserialize)]
struct Empty {}

/// Не generic по Runtime: приложение всегда на Wry (`tauri::Builder::default`),
/// а handle хранится в state конкретным типом.
pub fn init() -> TauriPlugin<Wry> {
    Builder::new(PLUGIN_NAME)
        .setup(|app, api| {
            let handle = api.register_android_plugin(KOTLIN_PACKAGE, KOTLIN_CLASS)?;
            app.manage(VpnPluginHandle(handle));
            Ok(())
        })
        .build()
}

async fn call<T: DeserializeOwned>(app: &AppHandle, command: &str, payload: impl Serialize) -> Result<T, String> {
    let plugin = app
        .try_state::<VpnPluginHandle>()
        .ok_or("VpnPlugin не зарегистрирован")?;
    plugin
        .0
        .run_mobile_plugin_async(command, payload)
        .await
        .map_err(|error| error.to_string())
}

pub async fn start(app: &AppHandle, args: StartArgs) -> Result<(), String> {
    call::<Empty>(app, "start", args).await.map(|_| ())
}

pub async fn stop(app: &AppHandle) -> Result<(), String> {
    call::<Empty>(app, "stop", ()).await.map(|_| ())
}

pub async fn stats(app: &AppHandle) -> Result<StatsResponse, String> {
    call(app, "stats", ()).await
}
