//! `NETunnelProviderManager` — загрузка/создание/сохранение конфигурации
//! VPN-профиля в системных настройках, плюс сборка `providerConfiguration`
//! для `.appex`. Используется из `connect.rs` (создание/сохранение перед
//! стартом), `disconnect.rs` и `stats.rs` (загрузка существующего перед
//! stop/опросом статистики).

use std::sync::mpsc;

use block2::RcBlock;
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2_foundation::{NSArray, NSDictionary, NSError, NSString};
use objc2_network_extension::NETunnelProviderManager;

use crate::config_gen::ExcludedRoutes;

/// Bundle identifier `.appex` из `macos-ext/VroxTunnelExtension` — должен
/// совпадать с `PRODUCT_BUNDLE_IDENTIFIER` в `macos-ext/project.yml`.
pub(super) const PROVIDER_BUNDLE_ID: &str = "com.vroxory.vpn.tunnel";

pub(super) fn nserror_to_string(err: &NSError) -> String {
    err.localizedDescription().to_string()
}

/// Загружает (или создаёт новый, если ни одного не сохранено)
/// `NETunnelProviderManager` для нашего provider bundle ID. Берём
/// `firstObject`, а не ищем конкретно по bundle ID среди нескольких —
/// `loadAllFromPreferencesWithCompletionHandler` возвращает только
/// конфигурации ТЕКУЩЕГО приложения, у нас всегда максимум одна.
///
/// Блокирующая (не `async`) — вызывается только внутри
/// `spawn_blocking`-замыканий выше по стеку, см. doc-комментарий модуля
/// `engine::macos`.
pub(super) fn load_or_create_manager_blocking() -> Result<Retained<NETunnelProviderManager>, String> {
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

pub(super) fn save_to_preferences_blocking(manager: &NETunnelProviderManager) -> Result<(), String> {
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

pub(super) fn load_from_preferences_blocking(manager: &NETunnelProviderManager) -> Result<(), String> {
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
pub(super) fn build_provider_configuration(
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
