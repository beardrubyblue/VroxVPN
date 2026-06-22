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
    NSArray, NSData, NSDictionary, NSError, NSNotification, NSNotificationCenter,
    NSOperationQueue, NSString,
};
use objc2_network_extension::{
    NETunnelProviderManager, NETunnelProviderProtocol, NETunnelProviderSession, NEVPNConnection,
    NEVPNStatus, NEVPNStatusDidChangeNotification,
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

    // ⚠ РЕАЛЬНЫЙ БАГ, подтверждён вживую (тоннель отключался сам через
    // ~2.5с на КАЖДОЙ попытке, "Stop command received" в логе
    // расширения почти сразу после "Calling startTunnelWithOptions"):
    // если читать connection.status() сразу после
    // startVPNTunnelAndReturnError(), система может ещё не успеть
    // обновить статус с прошлого .Disconnected на .Connecting — гонка
    // между синхронным возвратом из start-вызова и асинхронным
    // обновлением статуса через XPC. Старая версия этого кода доверяла
    // ПЕРВОМУ прочитанному статусу так же, как и статусам из
    // уведомлений, и мгновенно проваливала попытку на устаревшем
    // "Disconnected" от прошлой сессии, не дав тоннелю ни единого шанса
    // подняться. Исправлено: начальное значение не считается провалом —
    // только реальный статус, пришедший ЧЕРЕЗ уведомление (т.е.
    // подтверждённый переход, не устаревший снимок).
    let initial = unsafe { connection.status() };

    let result = (|| {
        if initial == NEVPNStatus::Connected {
            return Ok(());
        }
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(25);
        loop {
            let remaining = deadline.saturating_duration_since(std::time::Instant::now());
            if remaining.is_zero() {
                return Err("тоннель не поднялся за 25с (таймаут)".to_string());
            }
            let current = match rx.recv_timeout(remaining) {
                Ok(status) => status,
                Err(_) => return Err("тоннель не поднялся за 25с (таймаут)".to_string()),
            };
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

/// Наблюдатель за СОБСТВЕННЫМ разрывом соединения ПОСЛЕ удачного
/// коннекта — аналог Linux-варианта, который слушает `CommandEvent::
/// Terminated` осиротевшего pkexec-процесса и эмитит
/// `vpn-disconnected-unexpectedly`, если к этому моменту состояние
/// всё ещё `Connected` (не обычный disconnect, который сам переводит
/// слот в `Disconnecting`/`Idle`). Здесь источник события другой (NE
/// статус, не выход процесса), но контракт с фронтендом тот же.
///
/// САМОУДАЛЯЕТСЯ при первом же реальном разрыве (через
/// `removeObserver_name_object` на себя, токен хранится в `Arc<Mutex<>>`
/// — НЕ `Rc<RefCell>`: эта ячейка пишется на потоке, где регистрируется
/// обзёрвер (`spawn_blocking`-поток), а читается/обнуляется внутри
/// блока, который вызывается на главном потоке/run loop'е — то есть
/// двумя разными настоящими ОС-потоками. `RefCell`'овский флаг занятости
/// — не атомарный, гонка по нему была бы настоящей, даже если сам
/// objc2-объект внутри безопасен через ARC). Раньше токен/блок терялись
/// через `mem::forget` навсегда на КАЖДЫЙ успешный connect — за время
/// жизни приложения (много циклов connect/disconnect за одну сессию —
/// подтверждено вживую) это неограниченно растущая утечка: каждый
/// старый обзёрвер продолжает жить и реагировать на каждый последующий
/// статус, даже от уже совсем других соединений.
fn watch_for_unexpected_disconnect(app: AppHandle, connection: Retained<NEVPNConnection>) {
    let center = NSNotificationCenter::defaultCenter();
    let main_queue = NSOperationQueue::mainQueue();

    let token_cell: std::sync::Arc<
        std::sync::Mutex<
            Option<Retained<objc2::runtime::ProtocolObject<dyn objc2_foundation::NSObjectProtocol>>>,
        >,
    > = std::sync::Arc::new(std::sync::Mutex::new(None));
    let token_cell_for_block = token_cell.clone();

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
        // снимаем себя — задача наблюдателя на это соединение выполнена,
        // дальше его не зовут (на следующий connect зарегистрируется
        // новый, для нового connection-объекта).
        if let Some(tok) = token_cell_for_block.lock().unwrap().take() {
            unsafe {
                NSNotificationCenter::defaultCenter().removeObserver_name_object(
                    tok.as_ref(),
                    None,
                    None,
                )
            };
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
    *token_cell.lock().unwrap() = Some(token);
    // Сам блок (`RcBlock`) можно безопасно дать уронить здесь — Obj-C
    // `addObserverForName:...usingBlock:` копирует блок внутрь себя
    // (стандартная семантика Block-параметров), NSNotificationCenter
    // держит СВОЙ retain независимо от нашей Rust-обёртки. Раньше тоже
    // форсили `mem::forget(block)` на сам блок "на всякий случай" — не
    // нужно: `block` просто выходит из скоупа в конце функции, как
    // обычная Rust-переменная.
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

/// Счётчики трафика — запрос-ответ через `sendProviderMessage` к `.appex`
/// (см. `PacketTunnelProvider.swift::handleAppMessage` и
/// `netunnel.go::TunnelHandle.GetStats`). `manager.connection()` всегда
/// возвращает `NEVPNConnection`, но когда `protocolConfiguration` —
/// `NETunnelProviderProtocol` (как у нас), система реально создаёт
/// экземпляр подкласса `NETunnelProviderSession` (документировано Apple)
/// — `sendProviderMessage` объявлен только там. `Retained::downcast`
/// проверяет класс в рантайме (не просто слепой `cast_unchecked`),
/// поэтому безопасно вернуть осмысленную ошибку, если когда-нибудь это
/// окажется не так (например, до первого реального connect, когда
/// `connection()` может быть базовым `NEVPNConnection`).
///
/// Возвращает (tx_bytes, rx_bytes) — суммарно с момента старта тоннеля,
/// не дельту: дельту/скорость считает фронтенд между двумя опросами (см.
/// `commands.rs::get_traffic_totals`), как и для Linux-варианта.
fn get_traffic_totals_blocking() -> Result<(u64, u64), String> {
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
    Ok((tx_bytes, rx_bytes))
}

/// (upload_bytes, download_bytes) — суммарно с начала тоннеля. `spawn_blocking`
/// по той же причине, что и остальные NE-вызовы в этом файле (см.
/// doc-комментарий модуля): completion-блок `sendProviderMessage` зовётся
/// на главном run loop'е, синхронное ожидание ответа должно идти с
/// отдельного потока.
pub async fn get_traffic_totals() -> Result<(u64, u64), String> {
    tauri::async_runtime::spawn_blocking(get_traffic_totals_blocking)
        .await
        .map_err(|e| e.to_string())?
}

/// Killswitch на NE-пути — не отдельная операция: обеспечивается
/// `includedRoutes = [NEIPv4Route.default()]` в `NEPacketTunnelNetworkSettings`
/// (`PacketTunnelProvider.swift::startTunnel`) — весь трафик и так идёт
/// через тоннель, как только он реально поднят. `includeAllNetworks`
/// СОЗНАТЕЛЬНО НЕ используется (убран, см. `spawn_client_blocking` выше
/// и `docs/ARCHITECTURE.md`) — он блокирует и собственный трафик
/// расширения до того, как тоннель поднялся. Здесь no-op, а не заглушка
/// с ошибкой — `engine::enable_killswitch` вызывается best-effort уже
/// ПОСЛЕ удачного connect (см. commands.rs), так что он не должен мешать
/// или дублировать работу.
pub fn enable_killswitch(_app: &AppHandle, _vpn_server_host: &str) -> Result<(), String> {
    Ok(())
}

pub fn disable_killswitch(_app: &AppHandle) {}
