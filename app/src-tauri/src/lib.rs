mod app_update;
mod commands;
mod config_gen;
mod engine;
mod geoip;
mod geosite;
mod ping;
mod resources;
mod settings;
mod subscription;
mod tray;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // должен идти первым в цепочке — иначе плагин не успевает
        // перехватить повторный запуск до инициализации остального
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        // process — только для relaunch() после install_update_linux
        // (см. App.tsx::installUpdate). На macOS обновления идут через
        // TestFlight, этот плагин там не задействован.
        .plugin(tauri_plugin_process::init())
        .manage(engine::EngineState::default())
        .invoke_handler(tauri::generate_handler![
            commands::get_status,
            commands::connect,
            commands::disconnect,
            commands::get_traffic_totals,
            commands::fetch_servers,
            commands::update_geoip,
            commands::update_geosite,
            commands::get_settings,
            commands::set_setting,
            commands::ping_servers,
            commands::check_app_update,
            commands::install_update_linux,
            commands::quit_app,
            tray::sync_tray,
        ])
        .setup(|app| {
            // polkit-правило ставится при apt install (postinst.sh) — на
            // свежей установке это no-op без pkexec. Нужно ДО startup-
            // уборки ниже, иначе на апгрейде без переустановки она сама
            // спросит пароль раньше первого connect
            let _ = engine::ensure_polkit_rule(app.handle());
            // на случай, если предыдущий запуск приложения был убит/
            // крашнулся во время активного соединения или kill switch —
            // подчищаем осиротевший процесс vroxcore, TUN-интерфейс и
            // policy-drop nftables-таблицу, иначе пользователь рискует
            // остаться без сети до ручного вмешательства
            engine::cleanup_orphans(app.handle());
            engine::disable_killswitch(app.handle());
            tray::build_tray(app.handle())?;
            if let Some(window) = app.get_webview_window("main") {
                let window_clone = window.clone();
                window.on_window_event(move |event| {
                    // крестик окна сворачивает в трей вместо выхода —
                    // полный выход только через трей-меню или кнопку в
                    // настройках (commands::quit_app)
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        let _ = window_clone.hide();
                    }
                });
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
