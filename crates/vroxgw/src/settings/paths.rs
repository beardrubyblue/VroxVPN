//! Пути шлюза на файловой системе — единственное место, где они заданы.
//! Совпадают с раскладкой .deb (packaging/vroxgw/build-deb.sh).

/// Конфиг администратора.
pub const CONFIG_FILE: &str = "/etc/vroxgw/config.toml";
/// Наш форк hysteria2 (тот же бинарник, что sidecar Linux-приложения).
pub const VROXCORE_BIN: &str = "/usr/lib/vroxgw/vroxcore";
/// Встроенные снимки RU-списков (`geoip/`, `geosite/` внутри).
pub const BUNDLED_ROOT: &str = "/usr/share/vroxgw";
/// Постоянные данные: скачанные RU-списки, кеш подписки, выбор узла.
pub const STATE_ROOT: &str = "/var/lib/vroxgw";
/// Последняя удачно скачанная подписка — шлюз поднимается и тогда,
/// когда сервер подписки недоступен.
pub const SERVERS_CACHE: &str = "/var/lib/vroxgw/servers.json";
/// Узел, закреплённый `vroxgw switch <имя>`; нет файла — как в конфиге.
pub const NODE_OVERRIDE: &str = "/var/lib/vroxgw/node";
/// Временное (tmpfs): YAML с паролем сервера и состояние для `status`.
pub const RUN_DIR: &str = "/run/vroxgw";
pub const RUN_CONFIG: &str = "/run/vroxgw/config.yaml";
pub const STATE_FILE: &str = "/run/vroxgw/state.json";
