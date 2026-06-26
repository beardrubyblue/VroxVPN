//! Наблюдатель за неожиданным разрывом соединения ПОСЛЕ удачного
//! коннекта.

use objc2::rc::Retained;
use objc2_foundation::{NSNotification, NSNotificationCenter, NSOperationQueue};
use objc2_network_extension::{NEVPNConnection, NEVPNStatus, NEVPNStatusDidChangeNotification};
use tauri::{AppHandle, Emitter, Manager};

use super::wait_result::status_to_str;
use crate::engine::{EngineState, Slot};

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
type ObserverToken = Retained<objc2::runtime::ProtocolObject<dyn objc2_foundation::NSObjectProtocol>>;

// `ObserverToken` (objc2 `Retained<...>`) сам по себе не `Send`/`Sync` —
// clippy справедливо предупреждает, что `Arc` тут не даёт автоматической
// межпотоковой безопасности для содержимого. Это и не нужно: реальную
// безопасность даёт сам `Mutex` (эксклюзивный доступ ровно одного потока
// в момент времени — записывающего на spawn_blocking-потоке при
// регистрации, читающего на главном run loop'е при самоудалении), а не
// то, что `Retained` сам по себе можно безопасно расшарить без
// синхронизации. `Rc<RefCell<>>` не подходит именно потому, что доступ
// идёт с ДВУХ настоящих ОС-потоков (см. doc-комментарий функции выше).
#[allow(clippy::arc_with_non_send_sync)]
pub(super) fn watch_for_unexpected_disconnect(app: AppHandle, connection: Retained<NEVPNConnection>) {
    let center = NSNotificationCenter::defaultCenter();
    let main_queue = NSOperationQueue::mainQueue();

    let token_cell: std::sync::Arc<std::sync::Mutex<Option<ObserverToken>>> =
        std::sync::Arc::new(std::sync::Mutex::new(None));
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
