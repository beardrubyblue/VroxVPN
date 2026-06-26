//! Автообновление на Linux — свой механизм, не штатный
//! `tauri-plugin-updater`.

use std::path::Path;

use tauri::AppHandle;

use super::helper::run_helper;

/// Автообновление на Linux — свой механизм, не штатный `tauri-plugin-
/// updater`: тот умеет ставить только .app/.msi/.nsis/AppImage, а у нас
/// .deb (см. doc-комментарий на зависимости в Cargo.toml). Скачиваем
/// .deb по `download_url` из `version.json` (см. app_update.rs), сверяем
/// sha256 (целостность скачанного — version.json отдаётся по HTTPS, но
/// нет лишней проверки не помешает: сеть пользователя может быть кем-то
/// перехвачена даже под TLS, например корпоративным MITM-прокси с
/// добавленным в систему корневым сертификатом), ставим через тот же
/// privileged_helper.sh, что и весь остальной privileged-слой (никакого
/// нового способа получить root, кроме уже одобренного polkit-правила).
/// Перезапуск процесса — на стороне фронтенда (`relaunch()` из
/// `@tauri-apps/plugin-process`), одинаково для обеих платформ.
pub async fn install_update(
    app: &AppHandle,
    download_url: &str,
    expected_sha256: &str,
) -> Result<(), String> {
    let bytes = reqwest::get(download_url)
        .await
        .map_err(|e| format!("скачивание .deb: {e}"))?
        .bytes()
        .await
        .map_err(|e| format!("скачивание .deb: {e}"))?;

    if !expected_sha256.is_empty() {
        use sha2::{Digest, Sha256};
        let actual = hex::encode(Sha256::digest(&bytes));
        if !actual.eq_ignore_ascii_case(expected_sha256) {
            return Err(format!(
                "sha256 не совпадает: ожидали {expected_sha256}, получили {actual} — файл повреждён или подменён, обновление отменено"
            ));
        }
    }

    let dir = Path::new("/tmp/vroxory-vpn");
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let deb_path = dir.join("update.deb");
    std::fs::write(&deb_path, &bytes).map_err(|e| e.to_string())?;

    let deb_path_str = deb_path.to_string_lossy().to_string();
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || run_helper(&app, &["install-deb", &deb_path_str]))
        .await
        .map_err(|e| e.to_string())?
}
