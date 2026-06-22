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
use objc2_foundation::{
    NSArray, NSDictionary, NSError, NSNotification, NSNotificationCenter, NSOperationQueue,
    NSString,
};
use objc2_network_extension::{
    NETunnelProviderManager, NETunnelProviderProtocol, NEVPNConnection, NEVPNStatus,
    NEVPNStatusDidChangeNotification,
};
use tauri::{AppHandle, Emitter, Manager};

use crate::engine::{EngineState, Slot};

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

/// `startVPNTunnelAndReturnError` сам по себе НЕ подтверждает, что
/// тоннель реально поднялся — он успешен, если ОС приняла запрос на
/// старт (`status` переходит в `.Connecting`); реальный исход (успех
/// или провал `startTunnel` внутри расширения) приходит позже
/// асинхронно через `NEVPNStatusDidChangeNotification`. Подтверждено
/// вживую: без этого UI показывал "подключено" даже когда расширение
/// падало с ошибкой хендшейка — `spawn_client` считал успехом сам факт
/// вызова, не дожидаясь реального статуса (тот же класс бага, что уже
/// был исправлен для Linux через проверку "процесс не умер в первые
/// 1.5с" — здесь аналог через статус соединения, а не процесс, потому
/// что процесса, который мы сами породили, не существует).
///
/// Ждём терминального статуса (`.Connected` — успех, `.Disconnected`/
/// `.Invalid` — провал) с таймаутом. 25с — с запасом под QUIC-хендшейк
/// с retransmit'ами (наблюдалось до ~5с на провал в логах расширения),
/// не точная наука, можно скорректировать по реальным данным позже.
fn wait_for_connect_result_blocking(connection: &Retained<NEVPNConnection>) -> Result<(), String> {
    let (tx, rx) = mpsc::channel::<NEVPNStatus>();
    let center = NSNotificationCenter::defaultCenter();
    let main_queue = NSOperationQueue::mainQueue();

    // `addObserverForName_object_queue_usingBlock` принимает только
    // `'static`-блок (DynBlock без явного лайфтайма) — заимствование
    // `&NEVPNConnection` сюда не подходит, нужен честный `Retained`
    // (`.clone()` — просто ARC `retain`, не глубокое копирование).
    let connection_for_block = connection.clone();
    let observer_block = block2::RcBlock::new(move |_note: std::ptr::NonNull<NSNotification>| {
        let status = unsafe { connection_for_block.status() };
        let _ = tx.send(status);
    });

    let token = unsafe {
        center.addObserverForName_object_queue_usingBlock(
            Some(NEVPNStatusDidChangeNotification),
            None,
            Some(&main_queue),
            &observer_block,
        )
    };

    // статус мог уже измениться между startVPNTunnelAndReturnError и
    // регистрацией обзёрвера — проверяем текущий сразу, не только то,
    // что придёт через уведомление.
    let initial = unsafe { connection.status() };

    let result = (|| {
        let mut current = initial;
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(25);
        loop {
            match current {
                NEVPNStatus::Connected => return Ok(()),
                NEVPNStatus::Disconnected | NEVPNStatus::Invalid => {
                    return Err(format!(
                        "тоннель не поднялся (статус {})",
                        status_to_str(current)
                    ));
                }
                _ => {}
            }
            let remaining = deadline.saturating_duration_since(std::time::Instant::now());
            if remaining.is_zero() {
                return Err("тоннель не поднялся за 25с (таймаут)".to_string());
            }
            match rx.recv_timeout(remaining) {
                Ok(status) => current = status,
                Err(_) => return Err("тоннель не поднялся за 25с (таймаут)".to_string()),
            }
        }
    })();

    unsafe { center.removeObserver_name_object(token.as_ref(), None, None) };
    result
}

fn status_to_str(status: NEVPNStatus) -> &'static str {
    match status {
        NEVPNStatus::Invalid => "Invalid",
        NEVPNStatus::Disconnected => "Disconnected",
        NEVPNStatus::Connecting => "Connecting",
        NEVPNStatus::Connected => "Connected",
        NEVPNStatus::Reasserting => "Reasserting",
        NEVPNStatus::Disconnecting => "Disconnecting",
        _ => "Unknown",
    }
}

/// Долгоживущий наблюдатель за СОБСТВЕННЫМ разрывом соединения ПОСЛЕ
/// удачного коннекта — аналог Linux-варианта, который слушает
/// `CommandEvent::Terminated` осиротевшего pkexec-процесса и эмитит
/// `vpn-disconnected-unexpectedly`, если к этому моменту состояние
/// всё ещё `Connected` (не обычный disconnect, который сам переводит
/// слот в `Disconnecting`/`Idle`). Здесь источник события другой (NE
/// статус, не выход процесса), но контракт с фронтендом тот же.
/// Намеренно не снимается (`removeObserver`) — соединение на macOS
/// одно на всё приложение, обзёрвер живёт до конца процесса, лишний
/// `Box::leak`/эквивалент не страшнее, чем держать его в каком-то
/// глобальном состоянии specifically для одного снятия при выходе.
fn watch_for_unexpected_disconnect(app: AppHandle, connection: Retained<NEVPNConnection>) {
    let center = NSNotificationCenter::defaultCenter();
    let main_queue = NSOperationQueue::mainQueue();

    let block = block2::RcBlock::new(move |_note: std::ptr::NonNull<NSNotification>| {
        let status = unsafe { connection.status() };
        if !matches!(status, NEVPNStatus::Disconnected | NEVPNStatus::Invalid) {
            return;
        }
        let state = app.state::<EngineState>();
        let mut guard = state.0.lock().unwrap();
        if matches!(&*guard, Slot::Connected(_)) {
            *guard = Slot::Idle;
            drop(guard);
            let _ = app.emit("vpn-disconnected-unexpectedly", status_to_str(status));
        }
    });

    let token = unsafe {
        center.addObserverForName_object_queue_usingBlock(
            Some(NEVPNStatusDidChangeNotification),
            None,
            Some(&main_queue),
            &block,
        )
    };
    // блок и токен должны жить вечно (или до следующего connect) —
    // иначе ARC освободит блок, и NotificationCenter перестанет звать
    // обзёрвер при следующем же статусе.
    std::mem::forget(block);
    std::mem::forget(token);
}

/// Вся синхронная objc2-логика старта тоннеля — выполняется целиком на
/// одном blocking-потоке (см. doc-комментарий модуля). `app` нужен
/// здесь только для `watch_for_unexpected_disconnect` (excluded
/// routes/provider config уже посчитаны заранее, до перехода на
/// blocking-поток — `config_gen`-функциям нужен `&AppHandle` для
/// geoip/geosite, а сам `AppHandle` передаём по значению, он `Send`).
fn spawn_client_blocking(
    app: AppHandle,
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
        // includeAllNetworks СОЗНАТЕЛЬНО НЕ включаем — подтверждено
        // вживую (две полные потери интернета, требующие перезагрузки
        // Mac, прежде чем поняли причину): этот флаг блокирует ВЕСЬ
        // исходящий трафик системы, включая собственное соединение
        // расширения к VPN-серверу, сразу при переходе в "Connecting" —
        // до того, как setTunnelNetworkSettings вызван или startTunnel
        // завершился успехом. netunnel сам устанавливает UDP-соединение
        // к hysteria2-серверу ВНУТРИ startTunnel — то есть собственный
        // трафик тоннеля тоже блокируется этим флагом, и тоннель никогда
        // не поднимается. Документированная проблема Apple (chicken-
        // and-egg), не баг этого кода — Apple Developer Forums thread
        // 677102, wireguard-apple mailing list. Killswitch без него
        // слабее (только includedRoutes=[default] в
        // NEPacketTunnelNetworkSettings ПОСЛЕ удачного коннекта) —
        // пересмотреть отдельно, когда relay подтверждён рабочим.
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
    unsafe { connection.startVPNTunnelAndReturnError() }.map_err(|e| nserror_to_string(&e))?;

    wait_for_connect_result_blocking(&connection)?;
    watch_for_unexpected_disconnect(app, connection);
    Ok(())
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
    let app_for_blocking = app.clone();

    tauri::async_runtime::spawn_blocking(move || {
        spawn_client_blocking(app_for_blocking, &server, &config_json, &excluded, &inet4_addr, mtu)
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

/// `async` + `spawn_blocking` обязателен здесь, не просто для паритета
/// сигнатур (см. doc-комментарий модуля выше про `spawn_client`) — без
/// этого был реальный deadlock, подтверждённый вживую: `disconnect`
/// был обычной синхронной Tauri-командой, вызывавшей этот код прямо на
/// потоке диспетчера команд. `load_or_create_manager_blocking()` внутри
/// блокируется на `rx.recv()`, ожидая completion-callback от
/// `loadAllFromPreferencesWithCompletionHandler` — а тот callback
/// должен прийти через главный run loop приложения. Если поток
/// диспетчера команд и есть главный поток — он блокирует сам себя:
/// ждёт callback, который не может быть доставлен, потому что run loop
/// (на том же главном потоке) не крутится. Внешне это выглядело как
/// полный фриз всего приложения (не только кнопки) при попытке
/// disconnect — VPN-тоннель при этом РЕАЛЬНО отключался на уровне ОС
/// (`scutil --nc list` показывал Disconnected), просто Rust-сторона
/// никогда не получала об этом подтверждения и не возвращала ответ
/// фронтенду.
pub async fn kill_client(app: &AppHandle, config_path: &str) -> Result<(), String> {
    let _ = (app, config_path);
    tauri::async_runtime::spawn_blocking(kill_client_blocking)
        .await
        .map_err(|e| e.to_string())?
}

fn kill_client_blocking() -> Result<(), String> {
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
