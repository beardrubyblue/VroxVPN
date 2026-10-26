//! `vroxgw run` — сам шлюз (systemd-сервис). Цикл: подписка → узлы по
//! порядку (`nodes::rank`) → первый, через который прошёл трафик →
//! `session::run`, пока узел не упадёт → заново.

mod nodes;
mod session;
mod tunnel;

use std::time::Duration;

use crate::net::{forward, nat};
use crate::settings::{Config, State, Status};
use crate::shutdown::Shutdown;
use nodes::Candidate;
use session::End;
use tunnel::Tunnel;

pub use nodes::load_cache;

/// Пауза перед новой попыткой, если не поднялся ни один узел.
const RETRY_DELAY: Duration = Duration::from_secs(30);

pub async fn run() -> Result<(), String> {
    let config = Config::load()?;
    let mut shutdown = Shutdown::new()?;
    nat::install(&config.lan)?;
    // С kill switch форвардинг открывается только при живом тоннеле.
    if let Err(e) = forward::set_forwarding(!config.killswitch) {
        nat::remove();
        return Err(e);
    }

    supervise(&config, &mut shutdown).await;

    if let Err(e) = forward::set_forwarding(false) {
        eprintln!("[vroxgw] {e}");
    }
    nat::remove();
    Ok(())
}

/// Тоннель не работает — с kill switch закрываем форвардинг.
fn mark_down(config: &Config, state: State) {
    if config.killswitch {
        if let Err(e) = forward::set_forwarding(false) {
            eprintln!("[vroxgw] {e}");
        }
    }
    state.save();
}

/// Пробует узлы по порядку; первый, через который прошёл трафик, — наш.
async fn connect_first(config: &Config, last_failed: Option<&str>) -> Result<(Tunnel, Candidate), String> {
    let servers = nodes::fetch(&config.subscription_url).await?;
    let candidates = nodes::rank(servers, &config.preferred_node(), last_failed).await;
    let mut last_error = String::from("ни один узел не подключился");
    for candidate in candidates {
        let name = candidate.server.name.clone();
        eprintln!("[vroxgw] подключаюсь к «{name}»");
        State::new(Status::Connecting, Some(name.clone()), None).save();
        let mut tunnel = match Tunnel::spawn(&candidate.server, config.ru_bypass) {
            Ok(tunnel) => tunnel,
            Err(e) => {
                last_error = e;
                continue;
            }
        };
        match tunnel.wait_ready().await {
            Ok(()) => return Ok((tunnel, candidate)),
            Err(e) => {
                eprintln!("[vroxgw] «{name}»: {e}");
                last_error = format!("«{name}»: {e}");
                tunnel.stop().await;
            }
        }
    }
    Err(last_error)
}

async fn supervise(config: &Config, shutdown: &mut Shutdown) {
    let mut last_failed: Option<String> = None;
    loop {
        State::new(Status::Connecting, None, None).save();
        // Остановка во время перебора узлов: future отбрасывается, а с ним
        // и процесс vroxcore (kill_on_drop).
        let connected = tokio::select! {
            result = connect_first(config, last_failed.as_deref()) => result,
            _ = shutdown.recv() => return,
        };
        let (mut tunnel, candidate) = match connected {
            Ok(connected) => connected,
            Err(e) => {
                eprintln!("[vroxgw] {e}");
                mark_down(config, State::new(Status::Error, None, Some(e)));
                if shutdown.sleep(RETRY_DELAY).await {
                    return;
                }
                continue;
            }
        };

        let name = candidate.server.name.clone();
        if let Err(e) = forward::set_forwarding(true) {
            eprintln!("[vroxgw] {e}");
        }
        let mut state = State::new(Status::Connected, Some(name.clone()), None);
        state.latency_ms = candidate.latency_ms;
        state.save();
        eprintln!("[vroxgw] тоннель через «{name}» поднят");

        let end = session::run(config, &mut tunnel, &candidate.server.raw_uri, shutdown).await;
        tunnel.stop().await;
        match end {
            End::Shutdown => return,
            End::Failed(reason) => {
                eprintln!("[vroxgw] «{name}»: {reason}");
                last_failed = Some(candidate.server.raw_uri);
                mark_down(config, State::new(Status::Error, Some(name), Some(reason)));
            }
        }
    }
}
