//! Ожидание реального исхода подключения через
//! `NEVPNStatusDidChangeNotification` — `startVPNTunnelAndReturnError`
//! сам по себе ничего не подтверждает (см. doc-комментарий ниже).

use std::sync::mpsc;

use objc2::rc::Retained;
use objc2_foundation::{NSNotification, NSNotificationCenter, NSOperationQueue};
use objc2_network_extension::{NEVPNConnection, NEVPNStatus, NEVPNStatusDidChangeNotification};

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
pub(super) fn wait_for_connect_result_blocking(connection: &Retained<NEVPNConnection>) -> Result<(), String> {
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

pub(super) fn status_to_str(status: NEVPNStatus) -> &'static str {
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
