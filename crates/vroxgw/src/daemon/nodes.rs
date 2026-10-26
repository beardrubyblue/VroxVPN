//! Узлы подписки: загрузка с кешем на диске и порядок попыток подключения.

use vrox_core::ping::ping_all;
use vrox_core::subscription::{fetch_subscription, Server};

use crate::settings::paths::SERVERS_CACHE;
use crate::settings::NODE_AUTO;

const FETCH_TIMEOUT_SECS: u64 = 20;

/// Узел-кандидат и его пинг (нет — не ответил на ICMP, что для
/// hysteria2-сервера ещё не значит «мёртв»: UDP может работать).
pub struct Candidate {
    pub server: Server,
    pub latency_ms: Option<u32>,
}

pub fn load_cache() -> Result<Vec<Server>, String> {
    let text = std::fs::read_to_string(SERVERS_CACHE).map_err(|e| e.to_string())?;
    serde_json::from_str(&text).map_err(|e| e.to_string())
}

fn save_cache(servers: &[Server]) {
    let result = serde_json::to_string(servers)
        .map_err(|e| e.to_string())
        .and_then(|json| std::fs::write(SERVERS_CACHE, json).map_err(|e| e.to_string()));
    if let Err(e) = result {
        eprintln!("[vroxgw] не удалось сохранить кеш подписки: {e}");
    }
}

/// Подписка; сервер подписки недоступен — последний удачный список.
/// WireGuard-узлы пропускаем: шлюз умеет только hysteria2.
pub async fn fetch(url: &str) -> Result<Vec<Server>, String> {
    let fetched = fetch_subscription(url, FETCH_TIMEOUT_SECS).await.and_then(|(servers, _)| {
        let servers: Vec<Server> =
            servers.into_iter().filter(|server| server.wireguard.is_none()).collect();
        if servers.is_empty() {
            Err("в подписке нет узлов hysteria2".to_string())
        } else {
            Ok(servers)
        }
    });
    match fetched {
        Ok(servers) => {
            save_cache(&servers);
            Ok(servers)
        }
        Err(e) => {
            eprintln!("[vroxgw] подписка недоступна ({e}), беру кеш");
            load_cache().map_err(|_| format!("подписка недоступна: {e}; кеша нет"))
        }
    }
}

/// Порядок попыток: по пингу; закреплённый узел (не `auto`) — первым;
/// `avoid` (только что упавший) — последним, чтобы не крутиться на нём.
pub async fn rank(servers: Vec<Server>, preferred: &str, avoid: Option<&str>) -> Vec<Candidate> {
    let targets = servers.iter().map(|server| (server.name.clone(), server.host.clone())).collect();
    // join_all внутри ping_all сохраняет порядок — сопоставляем по индексу,
    // не по имени: имена узлов в подписке могут совпадать.
    let pings = ping_all(targets).await;
    let mut candidates: Vec<Candidate> = servers
        .into_iter()
        .zip(pings)
        .map(|(server, ping)| Candidate { server, latency_ms: ping.latency_ms })
        .collect();
    candidates.sort_by_key(|candidate| {
        let is_preferred = preferred != NODE_AUTO && candidate.server.name == preferred;
        let is_avoided = avoid == Some(candidate.server.raw_uri.as_str());
        (!is_preferred, is_avoided, candidate.latency_ms.unwrap_or(u32::MAX))
    });
    candidates
}
