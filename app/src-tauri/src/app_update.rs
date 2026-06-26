//! Проверка обновлений самого приложения — порт AppUpdater.check_update
//! из core/updater.py (ветка main), тот же version.json и тот же контракт
//! (основной URL + GitHub-фоллбек, если net.vroxory.com недоступен).

use serde::{Deserialize, Serialize};

const VERSION_URL: &str = "https://net.vroxory.com/vpn/version.json";
const VERSION_URL_FALLBACK: &str =
    "https://raw.githubusercontent.com/beardrubyblue/VroxVPN/main/version.json";
const CURRENT_VERSION: &str = "4.0.0";

#[derive(Deserialize)]
struct VersionJson {
    version: String,
    #[serde(default)]
    download_url: String,
    #[serde(default)]
    changelog: String,
    #[serde(default)]
    sha256: String,
}

#[derive(Serialize)]
pub struct UpdateCheck {
    pub current: String,
    pub latest: String,
    pub update_available: bool,
    pub download_url: String,
    pub changelog: String,
    pub sha256: String,
    /// На Linux фронтенд может сам установить найденное обновление
    /// (см. commands::install_update_linux — download .deb + privileged
    /// dpkg -i). На macOS обновления приходят через TestFlight — этой
    /// странице/кнопке там нечего делать, информируем и не предлагаем
    /// "установить" то, что некому ставить с нашей стороны.
    pub auto_installable: bool,
}

fn version_tuple(v: &str) -> Vec<u32> {
    v.trim()
        .trim_start_matches('v')
        .split('.')
        .map(|p| p.parse().unwrap_or(0))
        .collect()
}

pub async fn check_update(timeout_secs: u64) -> Result<UpdateCheck, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(timeout_secs))
        .build()
        .map_err(|e| e.to_string())?;

    let mut data: Option<VersionJson> = None;
    for url in [VERSION_URL, VERSION_URL_FALLBACK] {
        if let Ok(resp) = client.get(url).send().await {
            if let Ok(parsed) = resp.json::<VersionJson>().await {
                data = Some(parsed);
                break;
            }
        }
    }

    let data = data.ok_or("оба источника version.json недоступны")?;
    let update_available = version_tuple(&data.version) > version_tuple(CURRENT_VERSION);

    Ok(UpdateCheck {
        current: CURRENT_VERSION.to_string(),
        latest: data.version,
        update_available,
        download_url: data.download_url,
        changelog: data.changelog,
        sha256: data.sha256,
        auto_installable: cfg!(target_os = "linux"),
    })
}
