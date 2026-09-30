//! Узлы, добавленные вручную — второй источник серверов наряду с
//! подписками (группа «Added manually» на экране Nodes). Группа — это
//! источник, а протокол (Hysteria2 / WireGuard / AmneziaWG) — свойство
//! узла: одна ссылка hysteria2:// и .conf WireGuard живут вместе.

pub mod commands;
mod store;
