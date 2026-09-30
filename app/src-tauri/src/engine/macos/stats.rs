//! Счётчики трафика и память тоннеля — запрос-ответ через
//! `sendProviderMessage` к `.appex` (см.
//! `PacketTunnelProvider.swift::handleAppMessage` и
//! `netunnel.go::TunnelHandle.GetStats`).

use std::sync::mpsc;

use block2::RcBlock;
use objc2::rc::Retained;
use objc2_foundation::{NSData, NSError};
use objc2_network_extension::NETunnelProviderSession;
use tauri::AppHandle;

use super::manager::{load_or_create_manager_blocking, nserror_to_string};
use crate::engine::MemoryDebug;

/// `manager.connection()` всегда возвращает `NEVPNConnection`, но когда
/// `protocolConfiguration` — `NETunnelProviderProtocol` (как у нас),
/// система реально создаёт экземпляр подкласса `NETunnelProviderSession`
/// (документировано Apple) — `sendProviderMessage` объявлен только там.
/// `Retained::downcast` проверяет класс в рантайме (не просто слепой
/// `cast_unchecked`), поэтому безопасно вернуть осмысленную ошибку, если
/// когда-нибудь это окажется не так (например, до первого реального
/// connect, когда `connection()` может быть базовым `NEVPNConnection`).
///
/// Возвращает (tx_bytes, rx_bytes, rss_bytes, debug) — tx/rx суммарно с
/// момента старта тоннеля (не дельту: дельту/скорость считает фронтенд
/// между двумя опросами, см. `commands.rs::get_traffic_totals`, как и для
/// Linux-варианта), rss_bytes — текущая резидентная память ВСЕГО
/// процесса `.appex` (см. `PacketTunnelProvider.swift::currentRSSBytes`),
/// не только Go-кучи. `debug` — детальная разбивка (Go-куча/горутины/
/// relay), которую `.appex` уже кладёт в тот же JSON (см. `netunnel.go::
/// GetStats`), просто раньше Rust её не парсил.
fn get_traffic_totals_blocking() -> Result<(u64, u64, u64, Option<MemoryDebug>), String> {
    let manager = load_or_create_manager_blocking()?;
    let connection = unsafe { manager.connection() };
    let session = Retained::downcast::<NETunnelProviderSession>(connection)
        .map_err(|_| "connection не является NETunnelProviderSession (тоннель не запущен?)".to_string())?;

    let (tx, rx) = mpsc::channel::<Option<Retained<NSData>>>();
    let response_handler = RcBlock::new(move |data: *mut NSData| {
        // `data` приходит как autoreleased (стандартная ObjC-конвенция
        // для параметров блоков) — не наше владение, пока мы сами не
        // ретейним; `Retained::retain` именно это и делает (None для NULL).
        let owned = unsafe { Retained::retain(data) };
        let _ = tx.send(owned);
    });

    let message = NSData::with_bytes(b"getStats");
    let mut error: Option<Retained<NSError>> = None;
    let sent = unsafe {
        session.sendProviderMessage_returnError_responseHandler(
            &message,
            Some(&mut error),
            Some(&response_handler),
        )
    };
    if !sent {
        let msg = error
            .as_deref()
            .map(nserror_to_string)
            .unwrap_or_else(|| "sendProviderMessage вернул NO без error".to_string());
        return Err(format!("sendProviderMessage: {msg}"));
    }

    let response = rx
        .recv_timeout(std::time::Duration::from_secs(5))
        .map_err(|_| "getStats: таймаут ответа от .appex".to_string())?
        .ok_or_else(|| "getStats: .appex вернул пустой ответ (тоннель не активен?)".to_string())?;

    let json_str = std::str::from_utf8(&response.to_vec())
        .map_err(|e| format!("getStats: ответ не UTF-8: {e}"))?
        .to_string();
    let parsed: serde_json::Value =
        serde_json::from_str(&json_str).map_err(|e| format!("getStats: bad json: {e}"))?;
    let tx_bytes = parsed["txBytes"].as_u64().unwrap_or(0);
    let rx_bytes = parsed["rxBytes"].as_u64().unwrap_or(0);
    let rss_bytes = parsed["rssBytes"].as_u64().unwrap_or(0);
    let debug = MemoryDebug {
        heap_in_use: parsed["heapInUse"].as_u64().unwrap_or(0),
        heap_sys: parsed["heapSys"].as_u64().unwrap_or(0),
        goroutines: parsed["goroutines"].as_u64().unwrap_or(0),
        tcp_relays: parsed["tcpRelays"].as_u64().unwrap_or(0),
        udp_relays: parsed["udpRelays"].as_u64().unwrap_or(0),
        registry_size: parsed["registrySize"].as_u64().unwrap_or(0),
        avail_mem: parsed["availMem"].as_u64().unwrap_or(0),
        handshake_age_sec: parsed["handshakeAgeSec"].as_i64(),
        wg_error: parsed["wgError"].as_str().filter(|error| !error.is_empty()).map(String::from),
    };
    Ok((tx_bytes, rx_bytes, rss_bytes, Some(debug)))
}

/// (upload_bytes, download_bytes, memory_bytes) — суммарно с начала
/// тоннеля. `spawn_blocking` по той же причине, что и остальные NE-вызовы
/// в этом модуле (см. doc-комментарий `engine::macos`): completion-блок
/// `sendProviderMessage` зовётся на главном run loop'е, синхронное
/// ожидание ответа должно идти с отдельного потока. `_app`/`_config_path`
/// не используются на macOS (нужны только Linux-варианту, где нет
/// собственного API опроса процесса — см. `engine/linux.rs::
/// get_traffic_totals`), сигнатура общая ради единого вызова из
/// `commands.rs`.
pub async fn get_traffic_totals(
    _app: &AppHandle,
    _config_path: Option<&str>,
) -> Result<(u64, u64, u64, Option<MemoryDebug>), String> {
    tauri::async_runtime::spawn_blocking(get_traffic_totals_blocking)
        .await
        .map_err(|e| e.to_string())?
}
