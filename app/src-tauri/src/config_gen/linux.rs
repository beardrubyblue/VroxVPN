//! Linux: YAML-конфиг для sidecar `vroxcore` пишется файлом в
//! CONFIG_DIR (сам YAML — vrox_core::config_gen::generate_yaml, общий со
//! шлюзом vroxgw).

use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
use std::path::PathBuf;

use tauri::AppHandle;
use vrox_core::config_gen::generate_yaml;
use vrox_core::subscription::Server;

use crate::bypass;

const CONFIG_DIR: &str = "/tmp/vroxory-vpn";

fn safe_filename(name: &str) -> String {
    let mut result = String::new();
    let mut last_was_underscore = false;
    for c in name.chars() {
        let mapped = if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
            c
        } else {
            '_'
        };
        if mapped == '_' {
            if !last_was_underscore {
                result.push('_');
            }
            last_was_underscore = true;
        } else {
            result.push(mapped);
            last_was_underscore = false;
        }
    }
    let trimmed = result.trim_matches('_');
    if trimmed.is_empty() {
        "server".to_string()
    } else {
        trimmed.to_string()
    }
}

/// Создаёт CONFIG_DIR с правами 0700 (только текущий пользователь) —
/// конфиг внутри содержит пароль сервера в открытом виде. Отдельно
/// проверяет, что путь не подменили симлинком (классическая TOCTOU-
/// атака на предсказуемое имя в общем `/tmp`): если по этому пути
/// уже лежит симлинк — отказываемся следовать ему.
fn ensure_private_config_dir() -> Result<(), String> {
    if let Ok(meta) = std::fs::symlink_metadata(CONFIG_DIR) {
        if meta.file_type().is_symlink() {
            return Err(format!("{CONFIG_DIR} — симлинк, отказ"));
        }
        std::fs::set_permissions(CONFIG_DIR, std::fs::Permissions::from_mode(0o700))
            .map_err(|e| e.to_string())?;
        return Ok(());
    }
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(CONFIG_DIR)
        .map_err(|e| e.to_string())
}

/// Генерирует YAML конфиг для сервера и возвращает путь к файлу.
pub fn generate_config(
    app: &AppHandle,
    server: &Server,
    ru_bypass: bool,
) -> Result<PathBuf, String> {
    ensure_private_config_dir()?;
    let bypass = if ru_bypass { Some(bypass::load(app)?) } else { None };
    let yaml_str = generate_yaml(server, bypass)?;

    // суффикс-хэш от host:port — без него два сервера, чьи имена совпадают
    // после санитизации (например "Server #1" и "Server_#1" оба дают
    // "Server_1"), перезаписывали бы конфиг друг друга
    let filename = format!(
        "{}_{:x}.yaml",
        safe_filename(&server.name),
        {
            use std::hash::{Hash, Hasher};
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            (&server.host, server.port).hash(&mut hasher);
            hasher.finish() & 0xffff
        }
    );
    let path = PathBuf::from(CONFIG_DIR).join(filename);
    std::fs::write(&path, yaml_str).map_err(|e| e.to_string())?;
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))
        .map_err(|e| e.to_string())?;

    Ok(path)
}

