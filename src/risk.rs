//! Gestión de riesgo y circuit breakers

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use rust_decimal::prelude::{FromPrimitive, ToPrimitive};
use tracing::{warn, info};
use chrono::{Utc, NaiveDate};
use std::collections::VecDeque;

use crate::types::{Position, OrderRequest, RiskConfig, RiskStatus, Trade, OrderSide};

/// Gestor de riesgo principal
pub struct RiskManager {
    config: RiskConfig,
    initial_capital: Decimal,
    current_balance: Decimal,
    positions: Vec<Position>,
    trades: VecDeque<Trade>,
    daily_pnl: Decimal,
    current_date: NaiveDate,
    peak_balance: Decimal,
    status: RiskStatus,
    kill_switch_engaged: bool,
    total_volume: Decimal,
    winning_trades: u32,
    losing_trades: u32,
}

impl RiskManager {
    pub fn new(config: RiskConfig, initial_capital: Decimal) -> Self {
        let now = Utc::now().date_naive();
        Self {
            config,
            initial_capital,
            current_balance: initial_capital,
            positions: Vec::new(),
            trades: VecDeque::with_capacity(1000),
            daily_pnl: Decimal::ZERO,
            current_date: now,
            peak_balance: initial_capital,
            status: RiskStatus::Normal,
            kill_switch_engaged: false,
            total_volume: Decimal::ZERO,
            winning_trades: 0,
            losing_trades: 0,
        }
    }

    // =============================================
    // LÍMITES DINÁMICOS BASADOS EN CAPITAL
    // =============================================
    
    /// Obtener límite máximo de posición basado en capital actual
    fn get_max_position_usd(&self) -> Decimal {
        let max_position_pct = Decimal::from_f64(self.config.max_position_size_pct)
            .unwrap_or(dec!(0.15));
        (self.current_balance * max_position_pct).max(dec!(0.01))
    }
    
    /// Obtener límite diario de pérdidas basado en capital inicial
    fn get_max_daily_loss_usd(&self) -> Decimal {
        let max_loss_pct = Decimal::from_f64(self.config.max_daily_loss_pct)
            .unwrap_or(dec!(0.30));
        (self.initial_capital * max_loss_pct).max(dec!(0.01))
    }
    
    
    // =============================================
    // VERIFICACIÓN DE ÓRDENES
    // =============================================
    
    pub fn check_order(&mut self, order: &OrderRequest, current_price: Decimal) -> bool {
        if self.kill_switch_engaged {
            warn!("🔫 Kill switch engaged - orden rechazada");
            return false;
        }

        if self.current_balance <= Decimal::from_f64(0.50).unwrap() {
            warn!("⚠️ Balance crítico (${:.2}) - activando kill switch", self.current_balance);
            self.engage_kill_switch();
            return false;
        }

        let today = Utc::now().date_naive();
        if today != self.current_date {
            self.daily_pnl = Decimal::ZERO;
            self.current_date = today;
        }

        let order_value = order.quantity * current_price;
        let max_order_pct = Decimal::from_f64(0.20).unwrap();
        let max_order_value = self.current_balance * max_order_pct;
        
        if order_value > max_order_value {
            warn!("📊 Orden demasiado grande: ${:.2} > 20% del balance (${:.2})", 
                  order_value, max_order_value);
            return false;
        }

        let order_notional = order.quantity * current_price;
        let current_position = self.get_position(&order.symbol);
        let current_notional = current_position.map_or(Decimal::ZERO, |p| p.notional_value());
        
        let new_notional = match order.side {
            OrderSide::Buy => current_notional + order_notional,
            OrderSide::Sell => {
                if current_notional > order_notional {
                    current_notional - order_notional
                } else {
                    Decimal::ZERO
                }
            }
        };

        let max_position = self.get_max_position_usd();
        if new_notional > max_position {
            warn!("📊 Límite de posición excedido: ${:.2} > ${:.2} ({}% del balance)", 
                  new_notional, max_position, 
                  (max_position / self.current_balance * dec!(100)).to_f64().unwrap_or(0.0));
            return false;
        }

        let max_daily_loss = self.get_max_daily_loss_usd();
        if self.daily_pnl < -max_daily_loss {
            warn!("📉 Límite diario de pérdidas alcanzado: ${:.2} (máx: ${:.2})", 
                  self.daily_pnl, max_daily_loss);
            self.status = RiskStatus::ApproachingLimit;
            return false;
        }

        let current_drawdown = if self.peak_balance > Decimal::ZERO {
            (self.peak_balance - self.current_balance) / self.peak_balance
        } else {
            Decimal::ZERO
        };
        
        let max_drawdown = Decimal::from_f64(self.config.max_drawdown_pct).unwrap_or(dec!(0.15));
        
        if current_drawdown > max_drawdown {
            warn!("📉 Max drawdown alcanzado: {:.2}%", current_drawdown * dec!(100));
            if self.config.kill_switch_enabled {
                self.engage_kill_switch();
            }
            return false;
        }

        true
    }

    // =============================================
    // ACTUALIZACIÓN DE POSICIONES
    // =============================================
    
    pub fn update_position(&mut self, symbol: &str, size: Decimal, price: Decimal, order_id: String) {
        if self.current_balance < Decimal::from_f64(0.10).unwrap() && size < Decimal::ZERO {
            warn!("⚠️ Balance demasiado bajo para vender");
            return;
        }

        self.total_volume += size.abs() * price;
        
        let trade = Trade {
            symbol: symbol.to_string(),
            side: if size > Decimal::ZERO { OrderSide::Buy } else { OrderSide::Sell },
            size: size.abs(),
            price,
            timestamp: Utc::now(),
            pnl: Decimal::ZERO,
            order_id: order_id.clone(),
        };
        
        self.trades.push_front(trade);
        if self.trades.len() > 1000 {
            self.trades.pop_back();
        }
        
        if let Some(pos) = self.positions.iter_mut().find(|p| p.symbol == symbol) {
            if pos.size.is_zero() || (pos.size > Decimal::ZERO) == (size > Decimal::ZERO) {
                let total_size = pos.size + size;
                let total_cost = pos.size * pos.entry_price + size * price;
                pos.size = total_size;
                if !total_size.is_zero() {
                    pos.entry_price = total_cost / total_size;
                }
                pos.update_price(price);
            } else {
                let pnl = if size.abs() >= pos.size.abs() {
                    let pnl = pos.size * (price - pos.entry_price);
                    if pnl > Decimal::ZERO {
                        self.winning_trades += 1;
                    } else if pnl < Decimal::ZERO {
                        self.losing_trades += 1;
                    }
                    
                    if let Some(trade) = self.trades.front_mut() {
                        trade.pnl = pnl;
                    }
                    
                    pos.realized_pnl += pnl;
                    self.daily_pnl += pnl;
                    self.current_balance += pnl;
                    
                    pos.size = size + pos.size;
                    if !pos.size.is_zero() {
                        pos.entry_price = price;
                    }
                    pnl
                } else {
                    let pnl = size.abs() * (price - pos.entry_price);
                    if pnl > Decimal::ZERO {
                        self.winning_trades += 1;
                    } else if pnl < Decimal::ZERO {
                        self.losing_trades += 1;
                    }
                    
                    if let Some(trade) = self.trades.front_mut() {
                        trade.pnl = pnl;
                    }
                    
                    pos.realized_pnl += pnl;
                    self.daily_pnl += pnl;
                    self.current_balance += pnl;
                    
                    pos.size = pos.size + size;
                    pos.update_price(price);
                    pnl
                };
                
                info!("💰 P&L realizado: ${:.2} en {}", pnl, symbol);
            }
        } else {
            self.positions.push(Position::new(symbol, size, price));
        }

        self.update_balance();
        
        if self.current_balance > self.peak_balance {
            self.peak_balance = self.current_balance;
        }
    }

    pub fn get_position(&self, symbol: &str) -> Option<&Position> {
        self.positions.iter().find(|p| p.symbol == symbol)
    }
    
    pub fn get_position_mut(&mut self, symbol: &str) -> Option<&mut Position> {
        self.positions.iter_mut().find(|p| p.symbol == symbol)
    }

    fn update_balance(&mut self) {
        let mut total = self.initial_capital;
        for pos in &self.positions {
            total += pos.total_pnl();
        }
        self.current_balance = total.max(Decimal::ZERO);
    }

    pub fn update_position_price(&mut self, symbol: &str, price: Decimal) {
        if let Some(pos) = self.get_position_mut(symbol) {
            pos.update_price(price);
            self.update_balance();
        }
    }

    // =============================================
    // KILL SWITCH
    // =============================================
    
    pub fn engage_kill_switch(&mut self) {
        self.kill_switch_engaged = true;
        self.status = RiskStatus::KillSwitchEngaged;
        tracing::error!("🚨 KILL SWITCH ACTIVADO - Protegiendo capital restante: ${:.2}", self.current_balance);
    }

    pub fn is_kill_switch_engaged(&self) -> bool {
        self.kill_switch_engaged
    }

    pub fn status(&self) -> RiskStatus {
        self.status.clone()
    }

    // =============================================
    // GETTERS
    // =============================================
    
    pub fn current_balance(&self) -> Decimal {
        self.current_balance
    }

    pub fn peak_balance(&self) -> Decimal {
        self.peak_balance
    }

    pub fn initial_capital(&self) -> Decimal {
        self.initial_capital
    }

    pub fn daily_pnl(&self) -> Decimal {
        self.daily_pnl
    }

    pub fn total_pnl(&self) -> Decimal {
        self.current_balance - self.initial_capital
    }

    pub fn current_drawdown(&self) -> Decimal {
        if self.peak_balance > Decimal::ZERO {
            (self.peak_balance - self.current_balance) / self.peak_balance
        } else {
            Decimal::ZERO
        }
    }

    pub fn win_rate(&self) -> f64 {
        let total = self.winning_trades + self.losing_trades;
        if total > 0 {
            self.winning_trades as f64 / total as f64
        } else {
            0.0
        }
    }

    pub fn total_volume(&self) -> Decimal {
        self.total_volume
    }

    pub fn winning_trades(&self) -> u32 {
        self.winning_trades
    }

    pub fn losing_trades(&self) -> u32 {
        self.losing_trades
    }

    pub fn recent_trades(&self, count: usize) -> Vec<&Trade> {
        self.trades.iter().take(count).collect()
    }

    pub fn get_positions(&self) -> &Vec<Position> {
        &self.positions
    }
    
    pub fn max_position_usd(&self) -> Decimal {
        self.get_max_position_usd()
    }
    
    pub fn max_daily_loss_usd(&self) -> Decimal {
        self.get_max_daily_loss_usd()
    }
}