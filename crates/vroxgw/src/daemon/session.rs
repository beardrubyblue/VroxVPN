//! Работа с поднятым тоннелем: следим за процессом и связью, по таймеру
//! перечитываем подписку и RU-списки — без переподключения, пока узел
//! в подписке не изменился.

use std::time::Duration;

use vrox_core::bypass::{update_ru_cidrs, update_ru_domains};

use super::nodes;
use super::tunnel::Tunnel;
use crate::shutdown::Shutdown;
use crate::settings::paths::STATE_ROOT;
use crate::settings::Config;

const HEALTH_INTERVAL: Duration = Duration::from_secs(30);
/// Столько проверок подряд без связи — узел считаем мёртвым.
const HEALTH_FAILURES_LIMIT: u32 = 3;
const SECS_PER_HOUR: u64 = 3600;

pub enum End {
    Shutdown,
    Failed(String),
}

/// Новые RU-списки вступят в силу при следующем подключении: перезапуск
/// тоннеля ради них не стоит обрыва соединений у клиентов.
async fn refresh_bypass() {
    let root = std::path::Path::new(STATE_ROOT);
    if let Err(e) = update_ru_cidrs(root).await {
        eprintln!("[vroxgw] geoip не обновлён: {e}");
    }
    if let Err(e) = update_ru_domains(root).await {
        eprintln!("[vroxgw] geosite не обновлён: {e}");
    }
}

/// `Some` — узел пропал или изменился в подписке, пора переподключаться.
async fn refresh(config: &Config, raw_uri: &str) -> Option<String> {
    refresh_bypass().await;
    match nodes::fetch(&config.subscription_url).await {
        Ok(servers) if !servers.iter().any(|server| server.raw_uri == raw_uri) => {
            Some("узел изменился или пропал из подписки".into())
        }
        Ok(_) => None,
        Err(e) => {
            eprintln!("[vroxgw] {e}");
            None
        }
    }
}

pub async fn run(config: &Config, tunnel: &mut Tunnel, raw_uri: &str, shutdown: &mut Shutdown) -> End {
    let mut health = tokio::time::interval(HEALTH_INTERVAL);
    health.reset(); // первый тик сразу — лишний: тоннель только что проверен
    let refresh_every = Duration::from_secs(config.refresh_hours.max(1) * SECS_PER_HOUR);
    let mut refresh_timer = tokio::time::interval(refresh_every);
    refresh_timer.reset();
    let mut failures = 0;

    loop {
        tokio::select! {
            reason = tunnel.exited() => return End::Failed(reason),
            _ = shutdown.recv() => return End::Shutdown,
            _ = health.tick() => {
                if tunnel.probe().await {
                    failures = 0;
                } else {
                    failures += 1;
                    eprintln!("[vroxgw] нет связи через тоннель ({failures}/{HEALTH_FAILURES_LIMIT})");
                    if failures >= HEALTH_FAILURES_LIMIT {
                        return End::Failed("нет связи через тоннель".into());
                    }
                }
            }
            _ = refresh_timer.tick() => {
                if let Some(reason) = refresh(config, raw_uri).await {
                    return End::Failed(reason);
                }
            }
        }
    }
}
