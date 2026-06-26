//! Проверка обновлений самого приложения — порт AppUpdater.check_update
//! из core/updater.py (ветка main), тот же version.json и тот же контракт
//! (основной URL + GitHub-фоллбек, если net.vroxory.com недоступен).

#[cfg(not(target_os = "macos"))]
use serde::Deserialize;
use serde::Serialize;

#[cfg(not(target_os = "macos"))]
const VERSION_URL: &str = "https://net.vroxory.com/vpn/version.json";
#[cfg(not(target_os = "macos"))]
const VERSION_URL_FALLBACK: &str =
    "https://raw.githubusercontent.com/beardrubyblue/VroxVPN/main/version.json";
const CURRENT_VERSION: &str = "4.0.0";

#[cfg(not(target_os = "macos"))]
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

#[cfg(not(target_os = "macos"))]
fn version_tuple(v: &str) -> Vec<u32> {
    v.trim()
        .trim_start_matches('v')
        .split('.')
        .map(|p| p.parse().unwrap_or(0))
        .collect()
}

/// `version.json` в этом репозитории отдаёт `download_url` на `.deb` —
/// он описывает версию ТОЛЬКО Linux-сборки, не macOS (та версионируется
/// отдельно build-номерами в App Store Connect, см. `macos-ext/
/// build-testflight.sh::BUILD_NUMBER`). `CURRENT_VERSION` ниже — одна
/// Rust-константа, общая для обоих бинарников (компилируется из одного
/// исходника), поэтому сверять её с `version.json` имеет смысл только
/// на Linux: на macOS реальная версия может уйти вперёд (например, эта
/// сессия поправила пинг/добавила индикатор памяти без бампа этой
/// константы) без какой-либо связи с `version.json`, и сравнение с ним
/// показало бы либо ложное "доступно обновление", либо ложное "у вас
/// последняя версия" — ни то, ни другое не отражает реальность.
#[cfg(target_os = "macos")]
pub async fn check_update(_timeout_secs: u64) -> Result<UpdateCheck, String> {
    Ok(UpdateCheck {
        current: CURRENT_VERSION.to_string(),
        latest: CURRENT_VERSION.to_string(),
        update_available: false,
        download_url: String::new(),
        changelog: "Обновления на macOS приходят через TestFlight — отдельной проверки версии здесь нет".to_string(),
        sha256: String::new(),
        auto_installable: false,
    })
}

#[cfg(not(target_os = "macos"))]
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
