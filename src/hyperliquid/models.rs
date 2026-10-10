//! Modelos para Hyperliquid API (usando hypersdk)

use rust_decimal::Decimal;
use serde::{Serialize, Deserialize};
use std::collections::HashMap;

/// Tipo de orden
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HyperliquidOrderSide {
    Buy,
    Sell,
}

/// Respuesta de orden
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HyperliquidOrderResponse {
    pub success: bool,
    pub order_id: String,
    pub message: String,
}

/// Estado de la cuenta
#[derive(Debug, Clone)]
pub struct HyperliquidAccountState {
    pub balances: HashMap<String, Decimal>,
    pub wallet_address: String,
}