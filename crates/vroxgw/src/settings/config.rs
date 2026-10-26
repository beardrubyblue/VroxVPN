//! `/etc/vroxgw/config.toml` — то, что задаёт администратор.

use serde::Deserialize;

use super::paths::{CONFIG_FILE, NODE_OVERRIDE};

/// Значение `node`, при котором узел выбирается по пингу.
pub const NODE_AUTO: &str = "auto";

#[derive(Deserialize)]
pub struct Config {
    /// Ссылка на подписку (тот же формат, что в приложении).
    pub subscription_url: String,
    /// `"auto"` — лучший по пингу, иначе точное имя узла из подписки.
    #[serde(default = "default_node")]
    pub node: String,
    /// Российские IP и домены — напрямую, мимо тоннеля.
    #[serde(default = "default_true")]
    pub ru_bypass: bool,
    /// Тоннель лёг — клиенты LAN остаются без интернета, а не уходят
    /// напрямую в обход VPN.
    #[serde(default = "default_true")]
    pub killswitch: bool,
    /// Подсеть клиентов, трафик которых шлюз маскирует (NAT).
    #[serde(default = "default_lan")]
    pub lan: String,
    /// Как часто перечитывать подписку.
    #[serde(default = "default_refresh_hours")]
    pub refresh_hours: u64,
}

fn default_node() -> String {
    NODE_AUTO.into()
}
fn default_true() -> bool {
    true
}
fn default_lan() -> String {
    "192.168.1.0/24".into()
}
fn default_refresh_hours() -> u64 {
    6
}

impl Config {
    pub fn load() -> Result<Self, String> {
        let text = std::fs::read_to_string(CONFIG_FILE)
            .map_err(|e| format!("не удалось прочитать {CONFIG_FILE}: {e}"))?;
        let config: Self =
            toml::from_str(&text).map_err(|e| format!("ошибка в {CONFIG_FILE}: {e}"))?;
        if config.subscription_url.trim().is_empty() {
            return Err(format!("впишите subscription_url в {CONFIG_FILE}"));
        }
        Ok(config)
    }

    /// Узел с учётом `vroxgw switch`: закреплённый вручную важнее конфига.
    pub fn preferred_node(&self) -> String {
        std::fs::read_to_string(NODE_OVERRIDE)
            .map(|name| name.trim().to_string())
            .ok()
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| self.node.clone())
    }
}
