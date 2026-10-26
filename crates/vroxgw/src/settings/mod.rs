//! Конфиг администратора, пути и состояние демона.

mod config;
pub mod paths;
mod state;

pub use config::{Config, NODE_AUTO};
pub use state::{State, Status};
