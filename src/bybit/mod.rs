//! Módulo de integración con Bybit API

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

pub mod client;

pub use client::BybitClient;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum BybitOrderSide {
    Buy,
    Sell,
}

impl From<BybitOrderSide> for &str {
    fn from(side: BybitOrderSide) -> Self {
        match side {
            BybitOrderSide::Buy => "Buy",
            BybitOrderSide::Sell => "Sell",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum BybitOrderType {
    Limit,
    Market,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BybitPosition {
    pub symbol: String,
    pub size: Decimal,
    pub entry_price: Decimal,
    pub unrealized_pnl: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BybitBalance {
    pub asset: String,
    pub free: Decimal,
    pub locked: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BybitOrderRequest {
    pub symbol: String,
    pub side: BybitOrderSide,
    pub order_type: BybitOrderType,
    pub quantity: Decimal,
    pub price: Option<Decimal>,
    pub time_in_force: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BybitOrderResponse {
    pub order_id: String,
    pub success: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BybitTicker {
    pub symbol: String,
    pub last_price: Decimal,
    pub bid_price: Decimal,
    pub ask_price: Decimal,
    pub volume: Decimal,
}
