//! Modelos para persistencia

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::str::FromStr;

fn read_env_or_default<T: FromStr>(key: &str, default: T) -> T {
    std::env::var(key)
        .unwrap_or_default()
        .parse()
        .unwrap_or(default)
}

fn read_env_decimal(key: &str, default: &str) -> Decimal {
    Decimal::from_str(&std::env::var(key).unwrap_or_else(|_| default.to_string()))
        .unwrap_or_else(|_| Decimal::from_str(default).unwrap_or(Decimal::new(1, 1)))
}

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
            base_spread_ticks: read_env_or_default("STRATEGY_BASE_SPREAD_TICKS", 10),
            risk_aversion: read_env_decimal("STRATEGY_RISK_AVERSION", "0.15"),
            market_impact: read_env_decimal("STRATEGY_MARKET_IMPACT", "0.01"),
            inventory_skew_strength: read_env_decimal("STRATEGY_INVENTORY_SKEW_STRENGTH", "0.15"),
            volatility_lookback_minutes: read_env_or_default(
                "STRATEGY_VOLATILITY_LOOKBACK_MINUTES",
                20,
            ),
            order_refresh_ms: read_env_or_default("STRATEGY_ORDER_REFRESH_MS", 30000),
            max_inventory_ratio: read_env_decimal("STRATEGY_MAX_INVENTORY_RATIO", "0.10"),
        }
    }
}

/// Configuración de riesgo (en porcentajes)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskConfigDB {
    pub max_position_size_pct: Decimal, // % del capital
    pub max_daily_loss_pct: Decimal,    // % del capital
    pub max_drawdown_pct: Decimal,
    pub kill_switch_enabled: bool,
    pub circuit_breaker_threshold: Decimal,
}

impl Default for RiskConfigDB {
    fn default() -> Self {
        Self {
            max_position_size_pct: read_env_decimal("RISK_MAX_POSITION_SIZE_PCT", "0.10"),
            max_daily_loss_pct: read_env_decimal("RISK_MAX_DAILY_LOSS_PCT", "0.20"),
            max_drawdown_pct: read_env_decimal("RISK_MAX_DRAWDOWN_PCT", "0.15"),
            kill_switch_enabled: read_env_or_default("RISK_KILL_SWITCH_ENABLED", true),
            circuit_breaker_threshold: read_env_decimal("RISK_CIRCUIT_BREAKER_THRESHOLD", "0.03"),
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
        let bot_token = std::env::var("TELEGRAM_BOT_TOKEN").ok();
        let chat_id = std::env::var("TELEGRAM_CHAT_ID").ok();
        let enabled = bot_token.as_ref().map(|t| !t.is_empty()).unwrap_or(false)
            && chat_id.as_ref().map(|c| !c.is_empty()).unwrap_or(false);

        Self {
            enabled,
            bot_token,
            chat_id,
            notify_orders: read_env_or_default("TELEGRAM_NOTIFY_ORDERS", true),
            notify_fills: read_env_or_default("TELEGRAM_NOTIFY_FILLS", true),
            notify_status: read_env_or_default("TELEGRAM_NOTIFY_STATUS", true),
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
