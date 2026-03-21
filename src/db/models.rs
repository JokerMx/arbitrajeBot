//! Modelos para persistencia

use serde::{Serialize, Deserialize};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;

/// Configuración de estrategia
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyConfigDB {
    pub base_spread_ticks: i32,
    pub risk_aversion: Decimal,
    pub market_impact: Decimal,
    pub inventory_skew_strength: Decimal,
    pub volatility_lookback_minutes: i64,
    pub order_refresh_ms: i64,
    pub max_inventory_ratio: Decimal,
}

impl Default for StrategyConfigDB {
    fn default() -> Self {
        Self {
            base_spread_ticks: 8,
            risk_aversion: Decimal::new(25, 2),
            market_impact: Decimal::new(2, 2),
            inventory_skew_strength: Decimal::new(2, 1),
            volatility_lookback_minutes: 20,
            order_refresh_ms: 30000,
            max_inventory_ratio: Decimal::new(15, 2),
        }
    }
}

/// Configuración de riesgo (en porcentajes)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskConfigDB {
    pub max_position_size_pct: Decimal,   // % del capital
    pub max_daily_loss_pct: Decimal,      // % del capital
    pub max_drawdown_pct: Decimal,
    pub kill_switch_enabled: bool,
    pub circuit_breaker_threshold: Decimal,
}

impl Default for RiskConfigDB {
    fn default() -> Self {
        Self {
            max_position_size_pct: Decimal::new(15, 2),   // 0.15 (15%)
            max_daily_loss_pct: Decimal::new(30, 2),      // 0.30 (30%)
            max_drawdown_pct: Decimal::new(20, 2),        // 0.20 (20%)
            kill_switch_enabled: true,
            circuit_breaker_threshold: Decimal::new(5, 2), // 0.05 (5%)
        }
    }
}

/// Configuración de Telegram
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelegramConfigDB {
    pub enabled: bool,
    pub bot_token: Option<String>,
    pub chat_id: Option<String>,
    pub notify_orders: bool,
    pub notify_fills: bool,
    pub notify_status: bool,
}

impl Default for TelegramConfigDB {
    fn default() -> Self {
        Self {
            enabled: false,
            bot_token: None,
            chat_id: None,
            notify_orders: true,
            notify_fills: true,
            notify_status: true,
        }
    }
}

/// Configuración de mercado
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketConfigDB {
    pub symbol: String,
    pub enabled: bool,
    pub margin_alloc: Decimal,
    pub min_size: Decimal,
    pub price_decimals: i32,
    pub size_decimals: i32,
}

/// Configuración completa del sistema
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FullConfigDB {
    pub initial_capital: Decimal,
    pub strategy: StrategyConfigDB,
    pub risk: RiskConfigDB,
    pub markets: Vec<MarketConfigDB>,
    pub telegram: TelegramConfigDB,
    pub updated_at: Option<DateTime<Utc>>,
}