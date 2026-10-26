//! Общая логика vrox.vpn без UI и без Tauri: её используют и приложение
//! (app/src-tauri), и шлюз `vroxgw` (crates/vroxgw). Всё, что зависит от
//! платформы запуска (пути, запись файлов, NetworkExtension), остаётся у
//! вызывающей стороны.

pub mod bypass;
pub mod config_gen;
pub mod ping;
pub mod subscription;
pub mod wireguard;
