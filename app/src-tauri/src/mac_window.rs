//! iOS-сборка на Mac (isiOSAppOnMac): разрешить окно уже, чем по умолчанию.
//! macOS держит у iPad-приложения минимальную ширину окна крупнее
//! телефонной, а интерфейс у нас телефонный — окно нельзя было сузить.
//! `UIWindowScene.sizeRestrictions` работает только при запуске на Mac (на
//! iPad система его игнорирует) и считается пожеланием: система старается
//! его выполнить, но не обязана.

use objc2::MainThreadMarker;
use objc2_foundation::{NSProcessInfo, NSSize};
use objc2_ui_kit::{UIApplication, UIWindowScene};

/// Минимальный размер окна — самый узкий iPhone (mini, 360 pt), высота с
/// запасом под экран Shield (перетаскивание капсулы к кругу).
const MIN_WINDOW_WIDTH: f64 = 360.0;
const MIN_WINDOW_HEIGHT: f64 = 640.0;

/// Вызывать на главном потоке после появления окна (setup Tauri: tao к
/// этому моменту уже подключил сцену).
pub fn allow_narrow_window() {
    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };
    if !NSProcessInfo::processInfo().isiOSAppOnMac() {
        return;
    }
    for scene in UIApplication::sharedApplication(mtm).connectedScenes().iter() {
        let Some(window_scene) = scene.downcast_ref::<UIWindowScene>() else {
            continue;
        };
        if let Some(restrictions) = window_scene.sizeRestrictions() {
            restrictions.setMinimumSize(NSSize::new(MIN_WINDOW_WIDTH, MIN_WINDOW_HEIGHT));
        }
    }
}
