//! macOS-реализация — управление VPN через NetworkExtension.
//!
//! ЗАМЕНА прежнего sidecar+osascript+pf-подхода (удалён целиком, не
//! оставлен "на всякий случай" рядом — см. git log этого файла, если
//! нужно вернуться к нему). Тот путь реально заработал на живом Mac, но
//! был архитектурно временным мостиком: NetworkExtension всё равно
//! обязателен для iOS (см. docs/ARCHITECTURE.md), и решает на macOS то,
//! что sidecar-путь решить не мог (TCC, повторные пароли на каждый
//! привилегированный вызов, неподтверждённый killswitch через pf).
//!
//! Control-bridge к NEVPNManager/NETunnelProviderManager — крейт
//! `objc2-network-extension` (готовый, генерируемый из заголовков
//! NetworkExtension.framework, не пришлось писать ручные
//! `extern_class!`/`msg_send!` биндинги, как предполагалось в
//! ARCHITECTURE.md на момент написания плана).
//!
//! Важная деталь реализации: `Retained<NETunnelProviderManager>` и
//! `block2::RcBlock` НЕ `Send` — а Tauri требует `Send`-future от async
//! команд (`#[tauri::command] async fn connect`, см. commands.rs).
//! Поэтому вся объективно-цишная логика здесь синхронна (блокирующие
//! `std::sync::mpsc`-каналы вместо `tokio::sync::oneshot`/`.await`) и
//! выполняется целиком на одном выделенном потоке через
//! `tauri::async_runtime::spawn_blocking` — снаружи у `spawn_client`/
//! `kill_client` остаётся обычная асинхронная сигнатура (для паритета с
//! Linux-реализацией), но внутри await пересекает только `JoinHandle`,
//! результат которого (`Result<(), String>` и т.п.) — `Send`. Completion-
//! блоки NE вызываются на главном потоке (run loop приложения), фоновый
//! поток просто блокируется на `recv()`, ожидая результата — главный
//! поток в это время свободен крутить свой run loop как обычно.
//!
//! Сам `.appex` (NEPacketTunnelProvider, хост для `netunnel` через
//! gomobile) — отдельный Xcode-проект `macos-ext/` (Swift, не Rust) —
//! см. `docs/ARCHITECTURE.md`, раздел "Фаза 2".

use std::sync::mpsc;

use block2::RcBlock;
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2_foundation::{NSArray, NSDictionary, NSError, NSString};
use objc2_network_extension::{NETunnelProviderManager, NETunnelProviderProtocol};
use tauri::AppHandle;

use crate::config_gen::{self, ExcludedRoutes};
use crate::engine::ConnectionHandle;
use crate::subscription::Server;

/// Bundle identifier `.appex` из `macos-ext/VroxTunnelExtension` — должен
/// совпадать с `PRODUCT_BUNDLE_IDENTIFIER` в `macos-ext/project.yml`.
const PROVIDER_BUNDLE_ID: &str = "com.vroxory.vpn.tunnel";

fn nserror_to_string(err: &NSError) -> String {
    err.localizedDescription().to_string()
}

/// На Linux здесь пишется polkit-правило на весь жизненный цикл
/// приложения. NE не нуждается в отдельном шаге авторизации заранее —
/// разрешение даётся один раз при установке VPN-профиля через системный
/// диалог, не через privileged-helper.
pub fn ensure_polkit_rule(_app: &AppHandle) -> Result<(), String> {
    Ok(())
}

/// rp_filter — Linux-специфичный sysctl, аналога на macOS нет.
pub fn loosen_rp_filter(_app: &AppHandle) -> Result<(), String> {
    Ok(())
}

/// utun-интерфейс полностью под управлением ОС/NE-расширения — нет
/// отдельного шага "удалить интерфейс", как на Linux.
pub fn cleanup_interface(_app: &AppHandle) {}

/// Нет sidecar-процесса, который мог бы осиротеть при крахе приложения.
pub fn cleanup_orphans(_app: &AppHandle) {}

/// Загружает (или создаёт новый, если ни одного не сохранено)
/// `NETunnelProviderManager` для нашего provider bundle ID. Берём
/// `firstObject`, а не ищем конкретно по bundle ID среди нескольких —
/// `loadAllFromPreferencesWithCompletionHandler` возвращает только
/// конфигурации ТЕКУЩЕГО приложения, у нас всегда максимум одна.
///
/// Блокирующая (не `async`) — вызывается только внутри
/// `spawn_blocking`-замыканий ниже, см. doc-комментарий модуля.
fn load_or_create_manager_blocking() -> Result<Retained<NETunnelProviderManager>, String> {
    let (tx, rx) = mpsc::channel::<Result<Retained<NETunnelProviderManager>, String>>();

    let handler = RcBlock::new(
        move |managers: *mut NSArray<NETunnelProviderManager>, error: *mut NSError| {
            let result: Result<Retained<NETunnelProviderManager>, String> = unsafe {
                if let Some(err) = error.as_ref() {
                    Err(nserror_to_string(err))
                } else if let Some(arr) = managers.as_ref() {
                    Ok(arr.firstObject().unwrap_or_else(|| NETunnelProviderManager::new()))
                } else {
                    Ok(NETunnelProviderManager::new())
                }
            };
            let _ = tx.send(result);
        },
    );

    unsafe { NETunnelProviderManager::loadAllFromPreferencesWithCompletionHandler(&handler) };
    rx.recv().map_err(|_| "loadAllFromPreferences: канал закрыт".to_string())?
}

fn save_to_preferences_blocking(manager: &NETunnelProviderManager) -> Result<(), String> {
    let (tx, rx) = mpsc::channel::<Result<(), String>>();

    let handler = RcBlock::new(move |error: *mut NSError| {
        let result = unsafe {
            match error.as_ref() {
                Some(err) => Err(nserror_to_string(err)),
                None => Ok(()),
            }
        };
        let _ = tx.send(result);
    });

    unsafe { manager.saveToPreferencesWithCompletionHandler(Some(&handler)) };
    rx.recv().map_err(|_| "saveToPreferences: канал закрыт".to_string())?
}

fn load_from_preferences_blocking(manager: &NETunnelProviderManager) -> Result<(), String> {
    let (tx, rx) = mpsc::channel::<Result<(), String>>();

    let handler = RcBlock::new(move |error: *mut NSError| {
        let result = unsafe {
            match error.as_ref() {
                Some(err) => Err(nserror_to_string(err)),
                None => Ok(()),
            }
        };
        let _ = tx.send(result);
    });

    unsafe { manager.loadFromPreferencesWithCompletionHandler(&handler) };
    rx.recv().map_err(|_| "loadFromPreferences: канал закрыт".to_string())?
}

fn nsstring_array(items: &[String]) -> Retained<NSArray<NSString>> {
    let strings: Vec<Retained<NSString>> = items.iter().map(|s| NSString::from_str(s)).collect();
    let refs: Vec<&NSString> = strings.iter().map(|s| s.as_ref()).collect();
    NSArray::from_slice(&refs)
}

/// Собирает `providerConfiguration` — то, что `.appex` получит в
/// `protocolConfiguration.providerConfiguration` при старте тоннеля (см.
/// `macos-ext/VroxTunnelExtension/PacketTunnelProvider.swift::startTunnel`).
/// `configJSON` — формат `netunnel.Config`
/// (`packaging/hysteria2-patch/netunnel/netunnel.go`), строится
/// `config_gen::generate_provider_config_json`. `ipv4Exclude`/
/// `ipv6Exclude` — статическая часть Фазы 3 плана
/// (`config_gen::generate_excluded_routes`); Swift-сторона их пока не
/// читает (см. TODO в PacketTunnelProvider.swift) — передаём заранее,
/// чтобы не было второго похода сюда, когда Swift-часть Фазы 3 будет
/// готова.
fn build_provider_configuration(
    config_json: &str,
    excluded: &ExcludedRoutes,
    inet4_addr: &str,
    mtu: u32,
) -> Retained<NSDictionary<NSString, AnyObject>> {
    let keys = [
        NSString::from_str("configJSON"),
        NSString::from_str("inet4Addr"),
        NSString::from_str("mtu"),
        NSString::from_str("ipv4Exclude"),
        NSString::from_str("ipv6Exclude"),
    ];
    let key_refs: Vec<&NSString> = keys.iter().map(|k| k.as_ref()).collect();

    let config_json_ns = NSString::from_str(config_json);
    let inet4_addr_ns = NSString::from_str(inet4_addr);
    let mtu_ns = objc2_foundation::NSNumber::new_u32(mtu);
    let ipv4_exclude_ns = nsstring_array(&excluded.ipv4);
    let ipv6_exclude_ns = nsstring_array(&excluded.ipv6);

    let values: Vec<&AnyObject> = vec![
        config_json_ns.as_ref(),
        inet4_addr_ns.as_ref(),
        mtu_ns.as_ref(),
        ipv4_exclude_ns.as_ref(),
        ipv6_exclude_ns.as_ref(),
    ];

    NSDictionary::from_slices(&key_refs, &values)
}

/// Вся синхронная objc2-логика старта тоннеля — выполняется целиком на
/// одном blocking-потоке (см. doc-комментарий модуля). `app` здесь не
/// нужен (excluded routes/provider config уже посчитаны заранее, до
/// перехода на blocking-поток — `config_gen`-функциям нужен `&AppHandle`
/// для geoip/geosite, а `AppHandle` не `Send`-безопасен для произвольного
/// потока так же просто, поэтому считаем это на вызывающей стороне).
fn spawn_client_blocking(
    server: &Server,
    config_json: &str,
    excluded: &ExcludedRoutes,
    inet4_addr: &str,
    mtu: u32,
) -> Result<(), String> {
    let manager = load_or_create_manager_blocking()?;

    let proto = unsafe { NETunnelProviderProtocol::new() };
    unsafe {
        proto.setProviderBundleIdentifier(Some(&NSString::from_str(PROVIDER_BUNDLE_ID)));
        proto.setServerAddress(Some(&NSString::from_str(&server.host)));
        // killswitch под NE — основной механизм защиты от утечки трафика
        // при падении расширения (см. docs/ARCHITECTURE.md, Фаза 4),
        // замена pf-ruleset из удалённого sidecar-пути.
        proto.setIncludeAllNetworks(true);
        let config_dict = build_provider_configuration(config_json, excluded, inet4_addr, mtu);
        proto.setProviderConfiguration(Some(&config_dict));
    }

    unsafe {
        manager.setProtocolConfiguration(Some(&proto));
        manager.setLocalizedDescription(Some(&NSString::from_str(&format!(
            "vrox.vpn — {}",
            server.name
        ))));
        manager.setEnabled(true);
    }

    save_to_preferences_blocking(&manager)?;
    load_from_preferences_blocking(&manager)?;

    let connection = unsafe { manager.connection() };
    unsafe { connection.startVPNTunnelAndReturnError() }.map_err(|e| nserror_to_string(&e))
}

pub async fn spawn_client(
    app: &AppHandle,
    server: &Server,
    ru_bypass: bool,
) -> Result<(ConnectionHandle, String), String> {
    // config_gen-вызовам нужен &AppHandle (geoip/geosite) — считаем их
    // ДО перехода на blocking-поток, не внутри него.
    let excluded = config_gen::generate_excluded_routes(app, server, ru_bypass)?;
    let provider_config = config_gen::generate_provider_config_json(server);
    let config_json = serde_json::to_string(&provider_config).map_err(|e| e.to_string())?;
    let inet4_addr = provider_config["inet4Addr"]
        .as_str()
        .unwrap_or("100.100.100.101")
        .to_string();
    let mtu = provider_config["mtu"].as_u64().unwrap_or(1500) as u32;
    let server = server.clone();

    tauri::async_runtime::spawn_blocking(move || {
        spawn_client_blocking(&server, &config_json, &excluded, &inet4_addr, mtu)
    })
    .await
    .map_err(|e| e.to_string())??;

    // ConnectionHandle на macOS — `()` (см. engine.rs): нет процесса,
    // который мы сами породили, тоннель живёт в `.appex`, управляемом
    // ОС. Второй элемент кортежа (на Linux — путь к YAML-конфигу) здесь
    // не нужен — kill_client ниже заново грузит manager из system
    // preferences по bundle ID, не по этой строке.
    Ok(((), String::new()))
}

pub fn kill_client(_app: &AppHandle, _config_path: &str) -> Result<(), String> {
    let manager = load_or_create_manager_blocking()?;
    let connection = unsafe { manager.connection() };
    unsafe { connection.stopVPNTunnel() };
    Ok(())
}

/// Killswitch на NE-пути — не отдельная операция (см. doc-комментарий
/// модуля): включается как часть `NETunnelProviderProtocol.
/// includeAllNetworks` в `spawn_client` выше, до старта тоннеля. Здесь
/// no-op, а не заглушка с ошибкой — engine::enable_killswitch вызывается
/// best-effort уже ПОСЛЕ удачного connect (см. commands.rs), так что он
/// не должен мешать или дублировать работу.
pub fn enable_killswitch(_app: &AppHandle, _vpn_server_host: &str) -> Result<(), String> {
    Ok(())
}

pub fn disable_killswitch(_app: &AppHandle) {}
