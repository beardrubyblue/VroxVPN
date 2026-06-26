//! Linux-реализация привилегированного слоя — pkexec/polkit/nftables.
//! Логика connect/disconnect зеркалит core/tun_manager.py из ветки main
//! (тот же контракт privileged_helper.sh): kill идёт по пути конфига,
//! а не по pid процесса — pkexec отдаёт супервизору pid самой обёртки
//! pkexec, а не настоящего root-процесса hysteria2, поэтому убить его
//! напрямую по pid невозможно (см. helper-скрипт, секция kill-hysteria).
//!
//! Разбито на подмодули по концерну: `helper` (общая инфраструктура
//! pkexec-вызовов), `setup` (подготовка/уборка окружения),
//! `killswitch`, `connect`, `disconnect`, `stats`, `update`.

mod connect;
mod disconnect;
mod helper;
mod killswitch;
mod setup;
mod stats;
mod update;

pub use connect::spawn_client;
pub use disconnect::kill_client;
pub use killswitch::{disable_killswitch, enable_killswitch};
pub use setup::{cleanup_interface, cleanup_orphans, ensure_polkit_rule, loosen_rp_filter};
pub use stats::get_traffic_totals;
pub use update::install_update;

pub(super) const TUN_IFACE: &str = "tun-vroxory";
