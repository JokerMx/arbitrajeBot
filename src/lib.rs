// warp: la cadena de rutas .or() supera el límite de recursión por defecto
#![recursion_limit = "512"]
pub mod types;
pub mod strategy;
pub mod trend;
pub mod signals;
pub mod market;
pub mod risk;
pub mod executor;
pub mod bot;
pub mod notifier;
pub mod api;
pub mod telegram_bot;
pub mod db;
pub mod hyperliquid;
pub mod bybit;