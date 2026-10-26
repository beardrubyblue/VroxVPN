//! Сигналы остановки: SIGTERM (systemctl stop) и SIGINT (Ctrl+C при
//! ручном запуске `vroxgw run`).

use std::time::Duration;

use tokio::signal::unix::{signal, Signal, SignalKind};

pub struct Shutdown {
    term: Signal,
    interrupt: Signal,
}

impl Shutdown {
    pub fn new() -> Result<Self, String> {
        let term = signal(SignalKind::terminate()).map_err(|e| e.to_string())?;
        let interrupt = signal(SignalKind::interrupt()).map_err(|e| e.to_string())?;
        Ok(Self { term, interrupt })
    }

    pub async fn recv(&mut self) {
        tokio::select! {
            _ = self.term.recv() => {}
            _ = self.interrupt.recv() => {}
        }
    }

    /// `true` — пришёл сигнал остановки, пока ждали.
    pub async fn sleep(&mut self, delay: Duration) -> bool {
        tokio::select! {
            _ = tokio::time::sleep(delay) => false,
            _ = self.recv() => true,
        }
    }
}
