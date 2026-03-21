//! Tipos compartidos para el bot de market making

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

/// Side de la orden
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderSide {
    Buy,
    Sell,
}

impl std::fmt::Display for OrderSide {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OrderSide::Buy => write!(f, "COMPRA"),
            OrderSide::Sell => write!(f, "VENTA"),
        }
    }
}

/// Tipo de orden
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderType {
    Market,
    Limit,
}

/// Time in force
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TimeInForce {
    GoodTillCancel,
    ImmediateOrCancel,
    FillOrKill,
}

/// Estructura de posición
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub symbol: String,
    pub size: Decimal,
    pub entry_price: Decimal,
    pub current_price: Decimal,
    pub unrealized_pnl: Decimal,
    pub realized_pnl: Decimal,
    pub funding_pnl: Decimal,
    pub timestamp: DateTime<Utc>,
    pub liquidation_price: Option<Decimal>,
}

impl Position {
    pub fn new(symbol: &str, size: Decimal, entry_price: Decimal) -> Self {
        Self {
            symbol: symbol.to_string(),
            size,
            entry_price,
            current_price: entry_price,
            unrealized_pnl: Decimal::ZERO,
            realized_pnl: Decimal::ZERO,
            funding_pnl: Decimal::ZERO,
            timestamp: Utc::now(),
            liquidation_price: None,
        }
    }

    pub fn update_price(&mut self, price: Decimal) {
        self.current_price = price;
        if !self.size.is_zero() {
            self.unrealized_pnl = self.size * (price - self.entry_price);
        }
    }

    pub fn total_pnl(&self) -> Decimal {
        self.realized_pnl + self.unrealized_pnl + self.funding_pnl
    }

    pub fn notional_value(&self) -> Decimal {
        self.size.abs() * self.current_price
    }

    pub fn is_long(&self) -> bool {
        self.size > Decimal::ZERO
    }

    pub fn is_short(&self) -> bool {
        self.size < Decimal::ZERO
    }

    pub fn is_flat(&self) -> bool {
        self.size == Decimal::ZERO
    }
    
    pub fn close_position(&mut self, close_price: Decimal, close_size: Decimal) -> Decimal {
        let size_to_close = close_size.abs().min(self.size.abs());
        let pnl = size_to_close * (close_price - self.entry_price) * if self.is_long() { Decimal::ONE } else { -Decimal::ONE };
        
        self.realized_pnl += pnl;
        self.size = if self.is_long() {
            self.size - size_to_close
        } else {
            self.size + size_to_close
        };
        
        if self.size == Decimal::ZERO {
            self.entry_price = Decimal::ZERO;
        }
        
        pnl
    }
}

/// Request de orden
#[derive(Debug, Clone)]
pub struct OrderRequest {
    pub symbol: String,
    pub side: OrderSide,
    pub order_type: OrderType,
    pub quantity: Decimal,
    pub price: Option<Decimal>,
    pub reduce_only: bool,
    pub time_in_force: TimeInForce,
    pub client_order_id: Option<String>,
}

impl OrderRequest {
    pub fn limit(symbol: &str, side: OrderSide, quantity: Decimal, price: Decimal) -> Self {
        Self {
            symbol: symbol.to_string(),
            side,
            order_type: OrderType::Limit,
            quantity,
            price: Some(price),
            reduce_only: false,
            time_in_force: TimeInForce::GoodTillCancel,
            client_order_id: None,
        }
    }

    pub fn market(symbol: &str, side: OrderSide, quantity: Decimal) -> Self {
        Self {
            symbol: symbol.to_string(),
            side,
            order_type: OrderType::Market,
            quantity,
            price: None,
            reduce_only: false,
            time_in_force: TimeInForce::ImmediateOrCancel,
            client_order_id: None,
        }
    }

    pub fn reduce_only(mut self) -> Self {
        self.reduce_only = true;
        self
    }
}

/// Libro de órdenes nivel 2
#[derive(Debug, Clone)]
pub struct L2Book {
    pub coin: String,
    pub time: u64,
    pub bids: Vec<BookLevel>,
    pub asks: Vec<BookLevel>,
}

#[derive(Debug, Clone)]
pub struct BookLevel {
    pub px: Decimal,
    pub sz: Decimal,
    pub n: u64,
}

/// Configuración del mercado
#[derive(Debug, Clone, Deserialize)]
pub struct MarketConfig {
    pub symbol: String,
    pub enabled: bool,
    pub margin_alloc: f64,
    pub min_size: f64,
    pub price_decimals: u32,
    pub size_decimals: u32,
}

/// Configuración de estrategia
#[derive(Debug, Clone, Deserialize)]
pub struct StrategyConfig {
    pub name: String,
    pub base_spread_ticks: u32,
    pub risk_aversion: f64,
    pub market_impact: f64,
    pub inventory_skew_strength: f64,
    pub volatility_lookback_minutes: u64,
    pub order_refresh_time_ms: u64,
    pub max_inventory_ratio: f64,
}

/// Configuración de riesgo (en porcentajes)
#[derive(Debug, Clone, Deserialize)]
pub struct RiskConfig {
    pub max_position_size_pct: f64,      // % del capital actual
    pub max_daily_loss_pct: f64,         // % del capital inicial
    pub max_drawdown_pct: f64,
    pub kill_switch_enabled: bool,
    pub circuit_breaker_threshold: f64,
}

/// Estado de riesgo
#[derive(Debug, Clone, PartialEq)]
pub enum RiskStatus {
    Normal,
    ApproachingLimit,
    CircuitBreakerEngaged,
    KillSwitchEngaged,
}

/// Trade ejecutado
#[derive(Debug, Clone)]
pub struct Trade {
    pub symbol: String,
    pub side: OrderSide,
    pub size: Decimal,
    pub price: Decimal,
    pub timestamp: DateTime<Utc>,
    pub pnl: Decimal,
    pub order_id: String,
}