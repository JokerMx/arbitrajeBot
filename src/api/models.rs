//! Modelos para la API REST y WebSocket

use serde::{Deserialize, Serialize};

// =============================================
// MODELOS PARA ESTADÍSTICAS
// =============================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PositionResponse {
    pub symbol: String,
    pub size: f64,
    pub entry_price: f64,
    pub current_price: f64,
    pub unrealized_pnl: f64,
    pub realized_pnl: f64,
    pub value: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TradeResponse {
    pub id: String,
    pub symbol: String,
    pub side: String,
    pub size: f64,
    pub price: f64,
    pub value: f64,
    pub pnl: f64,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryPoint {
    pub timestamp: String,
    pub balance: f64,
    pub pnl: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatsResponse {
    pub balance: f64,
    pub pnl: f64,
    pub pnl_percent: f64,
    pub daily_pnl: f64,
    pub drawdown: f64,
    pub win_rate: f64,
    pub total_trades: u32,
    pub winning_trades: u32,
    pub losing_trades: u32,
    pub volume: f64,
    pub positions: Vec<PositionResponse>,
    pub trades: Vec<TradeResponse>,
    pub history: Vec<HistoryPoint>,
}

// =============================================
// MODELOS PARA CONFIGURACIÓN
// =============================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StrategyConfigResponse {
    pub base_spread_ticks: u32,
    pub risk_aversion: f64,
    pub market_impact: f64,
    pub inventory_skew_strength: f64,
    pub volatility_lookback_minutes: u64,
    pub order_refresh_ms: u64,
    pub max_inventory_ratio: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RiskConfigResponse {
    pub max_position_size_pct: f64, // Ahora es porcentaje
    pub max_daily_loss_pct: f64,    // Ahora es porcentaje
    pub max_drawdown_pct: f64,
    pub kill_switch_enabled: bool,
    pub circuit_breaker_threshold: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarketConfigResponse {
    pub symbol: String,
    pub enabled: bool,
    pub margin_alloc: f64,
    pub min_size: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TelegramConfigResponse {
    pub enabled: bool,
    pub bot_token: String,
    pub chat_id: String,
    pub notify_orders: bool,
    pub notify_fills: bool,
    pub notify_status: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FullConfigResponse {
    pub strategy: StrategyConfigResponse,
    pub risk: RiskConfigResponse,
    pub markets: Vec<MarketConfigResponse>,
    pub telegram: TelegramConfigResponse,
    pub initial_capital: f64,
    pub leverage: Vec<LeverageResponse>,
    pub protection: ProtectionResponse,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LeverageResponse {
    pub exchange: String,
    pub symbol: String,
    pub leverage: i32,
    pub max_leverage: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProtectionResponse {
    pub max_capital_at_risk_pct: f64,
    pub auto_stop_loss_enabled: bool,
    pub auto_stop_loss_pct: f64,
    pub max_leverage: i32,
    pub emergency_withdraw_enabled: bool,
}

// Todas las structs usan camelCase: es el formato que envía y espera el dashboard
// (antes eran snake_case y el backend ignoraba en silencio cada campo guardado).
// =============================================
// MODELOS PARA RECIBIR CONFIGURACIÓN
// =============================================

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateStrategyRequest {
    pub base_spread_ticks: Option<u32>,
    pub risk_aversion: Option<f64>,
    pub market_impact: Option<f64>,
    pub inventory_skew_strength: Option<f64>,
    pub volatility_lookback_minutes: Option<u64>,
    pub order_refresh_ms: Option<u64>,
    pub max_inventory_ratio: Option<f64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateRiskRequest {
    pub max_position_size_pct: Option<f64>, // Ahora es porcentaje
    pub max_daily_loss_pct: Option<f64>,    // Ahora es porcentaje
    pub max_drawdown_pct: Option<f64>,
    pub kill_switch_enabled: Option<bool>,
    pub circuit_breaker_threshold: Option<f64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateMarketRequest {
    pub symbol: String,
    pub enabled: Option<bool>,
    pub margin_alloc: Option<f64>,
    pub min_size: Option<f64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateTelegramRequest {
    pub enabled: Option<bool>,
    pub bot_token: Option<String>,
    pub chat_id: Option<String>,
    pub notify_orders: Option<bool>,
    pub notify_fills: Option<bool>,
    pub notify_status: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FullConfigUpdate {
    pub strategy: Option<UpdateStrategyRequest>,
    pub risk: Option<UpdateRiskRequest>,
    pub markets: Option<Vec<UpdateMarketRequest>>,
    pub telegram: Option<UpdateTelegramRequest>,
    pub leverage: Option<Vec<UpdateLeverageRequest>>,
    pub protection: Option<UpdateProtectionRequest>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct UpdateLeverageRequest {
    pub exchange: String,
    pub symbol: String,
    pub leverage: i32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProtectionRequest {
    pub max_capital_at_risk_pct: Option<f64>,
    pub auto_stop_loss_enabled: Option<bool>,
    pub auto_stop_loss_pct: Option<f64>,
    pub max_leverage: Option<i32>,
    pub emergency_withdraw_enabled: Option<bool>,
}

// =============================================
// MODELOS PARA WEBSOCKET
// =============================================

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", content = "data")]
pub enum WebSocketMessage {
    #[serde(rename = "stats")]
    Stats(StatsResponse),
    #[serde(rename = "trade")]
    Trade(TradeResponse),
    #[serde(rename = "config")]
    Config(FullConfigResponse),
    #[serde(rename = "alert")]
    Alert(String),
    #[serde(rename = "heartbeat")]
    Heartbeat,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum WebSocketClientMessage {
    #[serde(rename = "subscribe")]
    Subscribe { channel: String },
    #[serde(rename = "unsubscribe")]
    Unsubscribe { channel: String },
    #[serde(rename = "ping")]
    Ping,
}
