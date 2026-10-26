//! Процесс `vroxcore` (наш форк hysteria2) и проверка, что трафик
//! реально проходит через тоннель.

use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::time::Duration;

use tokio::process::{Child, Command};
use vrox_core::bypass::{load_ru_cidrs, load_ru_domains, BypassDirs};
use vrox_core::config_gen::{generate_yaml, RuBypass, TUN_NAME};
use vrox_core::subscription::Server;

use crate::settings::paths::{BUNDLED_ROOT, RUN_CONFIG, RUN_DIR, STATE_ROOT, VROXCORE_BIN};

/// Отвечает 204 без тела — дешёвая проверка «интернет через тоннель есть».
const PROBE_URL: &str = "http://cp.cloudflare.com/generate_204";
const PROBE_TIMEOUT: Duration = Duration::from_secs(5);
/// Сколько ждать первого успешного запроса через тоннель после запуска.
const READY_TIMEOUT: Duration = Duration::from_secs(20);
const READY_INTERVAL: Duration = Duration::from_secs(1);

pub struct Tunnel {
    child: Child,
    probe_client: reqwest::Client,
}

pub fn bypass_dirs() -> BypassDirs {
    BypassDirs { user_root: STATE_ROOT.into(), bundled_root: BUNDLED_ROOT.into() }
}

/// YAML с паролем сервера — в /run (tmpfs), файл сразу 0600: write +
/// chmod оставляли бы окно, когда он читается всеми. Сам каталог 0755 —
/// в нём же state.json для `vroxgw status` без sudo.
fn write_config(server: &Server, is_ru_bypass: bool) -> Result<(), String> {
    let bypass = is_ru_bypass.then(|| {
        let dirs = bypass_dirs();
        let (ipv4, ipv6) = load_ru_cidrs(&dirs);
        RuBypass { ipv4, ipv6, domains: load_ru_domains(&dirs) }
    });
    let yaml = generate_yaml(server, bypass)?;
    std::fs::create_dir_all(RUN_DIR).map_err(|e| e.to_string())?;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(RUN_CONFIG)
        .map_err(|e| format!("{RUN_CONFIG}: {e}"))?;
    file.write_all(yaml.as_bytes()).map_err(|e| e.to_string())
}

impl Tunnel {
    pub fn spawn(server: &Server, is_ru_bypass: bool) -> Result<Self, String> {
        write_config(server, is_ru_bypass)?;
        // stdout/stderr наследуются — лог vroxcore идёт в journald вместе
        // с логом демона (journalctl -u vroxgw).
        let child = Command::new(VROXCORE_BIN)
            .args(["client", "--config", RUN_CONFIG])
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| format!("не удалось запустить {VROXCORE_BIN}: {e}"))?;
        // Запрос привязан к TUN-интерфейсу (SO_BINDTODEVICE): без этого,
        // пока hysteria2 ещё не поставил маршруты, проверка ушла бы
        // напрямую через LAN и «успешно» прошла при мёртвом тоннеле.
        let probe_client = reqwest::Client::builder()
            .interface(TUN_NAME)
            .timeout(PROBE_TIMEOUT)
            .build()
            .map_err(|e| e.to_string())?;
        Ok(Self { child, probe_client })
    }

    pub async fn probe(&self) -> bool {
        match self.probe_client.get(PROBE_URL).send().await {
            Ok(response) => response.status() == reqwest::StatusCode::NO_CONTENT,
            Err(_) => false,
        }
    }

    /// Процесс жив и запрос через тоннель прошёл — иначе ошибка (QUIC-
    /// хендшейк не прошёл, неверный пароль, сервер недоступен).
    pub async fn wait_ready(&mut self) -> Result<(), String> {
        let deadline = tokio::time::Instant::now() + READY_TIMEOUT;
        while tokio::time::Instant::now() < deadline {
            if let Ok(Some(status)) = self.child.try_wait() {
                return Err(format!("vroxcore завершился сразу после запуска ({status})"));
            }
            if self.probe().await {
                return Ok(());
            }
            tokio::time::sleep(READY_INTERVAL).await;
        }
        Err("тоннель не пропустил трафик за отведённое время".into())
    }

    /// Ждёт, пока vroxcore не завершится сам.
    pub async fn exited(&mut self) -> String {
        match self.child.wait().await {
            Ok(status) => format!("vroxcore завершился ({status})"),
            Err(e) => format!("vroxcore: {e}"),
        }
    }

    pub async fn stop(mut self) {
        let _ = self.child.kill().await;
    }
}
