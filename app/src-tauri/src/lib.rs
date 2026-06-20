mod commands;
mod config_gen;
mod engine;
mod subscription;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_shell::init())
        .manage(engine::EngineState::default())
        .invoke_handler(tauri::generate_handler![
            commands::get_status,
            commands::connect,
            commands::disconnect,
            commands::engine_version,
            commands::fetch_servers,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
