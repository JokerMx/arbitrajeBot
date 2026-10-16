//! Gestión de riesgo y circuit breakers

use chrono::{NaiveDate, Utc};
use rust_decimal::prelude::{FromPrimitive, ToPrimitive};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use std::collections::VecDeque;
use tracing::{info, warn};

use crate::db::models::ProtectionConfigDB;
use crate::types::{OrderRequest, OrderSide, Position, RiskConfig, RiskStatus, Trade};

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
    // Nuevos campos para protección
    protection: ProtectionConfigDB,
    leverage_configs: std::collections::HashMap<String, i32>,
    stop_loss_triggered: bool,
    /// Equity al inicio del día (para el P&L diario con datos reales del exchange)
    day_start_balance: Decimal,
    /// `true` una vez que el balance se sincronizó con el exchange
    synced_with_exchange: bool,
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
            protection: ProtectionConfigDB::default(),
            leverage_configs: std::collections::HashMap::new(),
            stop_loss_triggered: false,
            day_start_balance: initial_capital,
            synced_with_exchange: false,
        }
    }

    /// Cambia los límites de riesgo en vivo (desde el dashboard)
    pub fn set_config(&mut self, config: RiskConfig) {
        info!(
            "🛡️ Límites de riesgo: pérdida diaria {:.1}%, drawdown máx {:.1}%, kill switch {}",
            config.max_daily_loss_pct * 100.0,
            config.max_drawdown_pct * 100.0,
            config.kill_switch_enabled
        );
        self.config = config;
    }

    pub fn set_protection(&mut self, protection: ProtectionConfigDB) {
        let max_risk = protection.max_capital_at_risk_pct.to_f64().unwrap_or(0.0) * 100.0;
        let stop_loss = protection.auto_stop_loss_pct.to_f64().unwrap_or(0.0) * 100.0;
        info!(
            "🛡️ Configuración de protección actualizada: max_risk={:.1}%, stop_loss={:.1}%",
            max_risk, stop_loss
        );
        self.protection = protection;
    }

    pub fn set_leverage(&mut self, exchange: &str, symbol: &str, leverage: i32) {
        let key = format!("{}_{}", exchange, symbol);
        let max = self.protection.max_leverage as i32;
        let leverage = leverage.max(1).min(max);
        self.leverage_configs.insert(key, leverage);
        info!("📈 Apalancamiento {} {}: {}x", exchange, symbol, leverage);
    }

    pub fn get_leverage(&self, exchange: &str, symbol: &str) -> i32 {
        let key = format!("{}_{}", exchange, symbol);
        self.leverage_configs.get(&key).copied().unwrap_or(1)
    }

    fn check_capital_protection(&self) -> (bool, String) {
        let capital_at_risk = self.calculate_capital_at_risk();
        let max_allowed = self.initial_capital * self.protection.max_capital_at_risk_pct;

        if capital_at_risk > max_allowed {
            let pct = (capital_at_risk / self.initial_capital * dec!(100))
                .to_f64()
                .unwrap_or(0.0);
            return (
                false,
                format!(
                    "⚠️ Capital en riesgo: ${:.2} ({}%) > Máximo: ${:.2}",
                    capital_at_risk, pct, max_allowed
                ),
            );
        }

        (true, "".to_string())
    }

    fn calculate_capital_at_risk(&self) -> Decimal {
        let mut total = Decimal::ZERO;
        for pos in &self.positions {
            let position_value = pos.size * pos.current_price;
            let leverage_val = self.get_leverage("simulated", &pos.symbol) as f64;
            let leveraged_value =
                position_value * Decimal::from_f64(leverage_val).unwrap_or(dec!(1));
            total = total + leveraged_value;
        }
        total
    }

    fn check_stop_loss(&self) -> (bool, String) {
        if !self.protection.auto_stop_loss_enabled {
            return (true, "".to_string());
        }

        let pnl_pct = (self.current_balance - self.initial_capital) / self.initial_capital;
        let stop_loss_threshold = self.protection.auto_stop_loss_pct;
        let hundred = Decimal::from(100);

        if pnl_pct < -stop_loss_threshold {
            return (
                false,
                format!(
                    "🛑 STOP LOSS: Pérdida {}% > Umbral {}%",
                    (-pnl_pct * hundred).to_f64().unwrap_or(0.0),
                    (stop_loss_threshold * hundred).to_f64().unwrap_or(0.0)
                ),
            );
        }

        (true, "".to_string())
    }

    pub fn validate_order_with_protection(
        &mut self,
        exchange: &str,
        symbol: &str,
        size: Decimal,
        price: Decimal,
    ) -> (bool, String) {
        let leverage = self.get_leverage(exchange, symbol);
        let order_value = size * price;
        let margin_used = order_value / Decimal::from_f64(leverage as f64).unwrap_or(dec!(1));

        let max_leverage = self.protection.max_leverage as i32;
        if leverage > max_leverage {
            return (
                false,
                format!(
                    "⚠️ Apalancamiento {}x excede máximo {}x",
                    leverage, max_leverage
                ),
            );
        }

        let max_capital_at_risk = (self.initial_capital
            * Decimal::from_f64(
                self.protection
                    .max_capital_at_risk_pct
                    .to_f64()
                    .unwrap_or(0.25),
            )
            .unwrap_or(dec!(12.50)))
        // Mínimo de Hyperliquid ($10) + margen para redondeo de precio/tamaño
        .max(dec!(11));

        if margin_used > max_capital_at_risk {
            return (
                false,
                format!(
                    "⚠️ Margen ${:.2} > Máximo riesgo ${:.2}",
                    margin_used, max_capital_at_risk
                ),
            );
        }

        let (ok, msg) = self.check_capital_protection();
        if !ok {
            return (false, msg);
        }

        let (ok, msg) = self.check_stop_loss();
        if !ok {
            self.stop_loss_triggered = true;
            return (false, msg);
        }

        if self.stop_loss_triggered {
            return (
                false,
                "🛑 Stop loss activado - no se permiten nuevas órdenes".to_string(),
            );
        }

        (true, "".to_string())
    }

    // =============================================
    // LÍMITES DINÁMICOS BASADOS EN CAPITAL
    // =============================================

    /// Obtener límite máximo de posición basado en capital actual y apalancamiento
    fn get_max_position_usd(&self) -> Decimal {
        let max_position_pct =
            Decimal::from_f64(self.config.max_position_size_pct).unwrap_or(dec!(0.15));
        let base_max = (self.current_balance * max_position_pct).max(dec!(0.01));
        let leverage = self.get_leverage("simulated", "BTC");
        base_max * Decimal::from_f64(leverage as f64).unwrap_or(dec!(1))
    }

    /// Obtener límite máximo de posición para un símbolo específico con su apalancamiento
    pub fn get_max_position_for_symbol(&self, exchange: &str, symbol: &str) -> Decimal {
        let max_position_pct =
            Decimal::from_f64(self.config.max_position_size_pct).unwrap_or(dec!(0.15));
        let base_max = (self.current_balance * max_position_pct).max(dec!(0.01));
        let leverage = self.get_leverage(exchange, symbol);
        base_max * Decimal::from_f64(leverage as f64).unwrap_or(dec!(1))
    }

    /// Obtener límite diario de pérdidas basado en capital inicial
    fn get_max_daily_loss_usd(&self) -> Decimal {
        let max_loss_pct = Decimal::from_f64(self.config.max_daily_loss_pct).unwrap_or(dec!(0.30));
        (self.initial_capital * max_loss_pct).max(dec!(0.01))
    }

    // =============================================
    // VERIFICACIÓN DE ÓRDENES
    // =============================================

    pub fn check_order(&mut self, order: &OrderRequest, current_price: Decimal) -> bool {
        // Una orden reduce-only solo puede achicar la posición: siempre se permite
        if order.reduce_only {
            return true;
        }

        if self.kill_switch_engaged {
            warn!("🔫 Kill switch engaged - orden rechazada");
            return false;
        }

        if self.current_balance <= Decimal::from_f64(0.50).unwrap() {
            warn!(
                "⚠️ Balance crítico (${:.2}) - activando kill switch",
                self.current_balance
            );
            self.engage_kill_switch();
            return false;
        }

        let exchange = "hyperliquid";

        let (valid, msg) = self.validate_order_with_protection(
            exchange,
            &order.symbol,
            order.quantity,
            current_price,
        );

        if !valid {
            warn!("{}", msg);
            return false;
        }

        let today = Utc::now().date_naive();
        if today != self.current_date {
            self.daily_pnl = Decimal::ZERO;
            self.current_date = today;
        }

        let order_value = order.quantity * current_price;
        let max_order_pct = Decimal::from_f64(0.50).unwrap();
        let base_limit = (self.initial_capital * max_order_pct).max(dec!(11));
        let max_order_value = base_limit;

        if order_value > max_order_value {
            warn!(
                "📊 Orden demasiado grande: ${:.2} > 50% del capital (${:.2})",
                order_value, max_order_value
            );
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

        let leverage = self.get_leverage(exchange, &order.symbol);
        let max_position = self.current_balance
            * Decimal::from_f64(self.config.max_position_size_pct).unwrap_or(dec!(0.25));
        let max_position_with_lev =
            max_position * Decimal::from_f64(leverage as f64).unwrap_or(dec!(1));

        if new_notional > max_position_with_lev && leverage > 1 {
            warn!(
                "📊 Límite de posición con apalancamiento excedido: ${:.2} > ${:.2} ({}x)",
                new_notional, max_position_with_lev, leverage
            );
            return false;
        }

        let max_daily_loss = self.get_max_daily_loss_usd();
        if self.daily_pnl < -max_daily_loss {
            warn!(
                "📉 Límite diario de pérdidas alcanzado: ${:.2} (máx: ${:.2})",
                self.daily_pnl, max_daily_loss
            );
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
            warn!(
                "📉 Max drawdown alcanzado: {:.2}%",
                current_drawdown * dec!(100)
            );
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

    pub fn update_position(
        &mut self,
        symbol: &str,
        size: Decimal,
        price: Decimal,
        order_id: String,
    ) {
        if self.current_balance < Decimal::from_f64(0.10).unwrap() && size < Decimal::ZERO {
            warn!("⚠️ Balance demasiado bajo para vender");
            return;
        }

        self.total_volume += size.abs() * price;

        let trade = Trade {
            symbol: symbol.to_string(),
            side: if size > Decimal::ZERO {
                OrderSide::Buy
            } else {
                OrderSide::Sell
            },
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

    /// Sincroniza el balance con la equity REAL del exchange.
    /// La primera sincronización fija el capital inicial a la equity real, para que
    /// stop-loss y drawdown se midan contra el dinero que realmente hay en la cuenta.
    pub fn sync_balance(&mut self, equity: Decimal) {
        if equity <= Decimal::ZERO {
            return;
        }
        if !self.synced_with_exchange {
            if self.initial_capital != equity {
                info!(
                    "🔄 Capital inicial ajustado a equity real: ${:.2} (config: ${:.2})",
                    equity, self.initial_capital
                );
            }
            self.initial_capital = equity;
            self.peak_balance = equity;
            self.day_start_balance = equity;
            self.synced_with_exchange = true;
        }

        let today = Utc::now().date_naive();
        if today != self.current_date {
            self.current_date = today;
            self.day_start_balance = equity;
        }

        self.current_balance = equity;
        self.daily_pnl = equity - self.day_start_balance;
        if equity > self.peak_balance {
            self.peak_balance = equity;
        }
    }

    /// `true` si se debe dejar de abrir posiciones y cerrar lo abierto
    /// (kill switch, stop loss, pérdida diaria o drawdown máximo).
    pub fn should_flatten(&mut self) -> bool {
        if self.kill_switch_engaged || self.stop_loss_triggered {
            return true;
        }
        let (ok, msg) = self.check_stop_loss();
        if !ok {
            warn!("{}", msg);
            self.stop_loss_triggered = true;
            return true;
        }
        if self.daily_pnl < -self.get_max_daily_loss_usd() {
            warn!("📉 Límite diario de pérdidas alcanzado: ${:.2}", self.daily_pnl);
            self.status = RiskStatus::ApproachingLimit;
            return true;
        }
        let max_drawdown = Decimal::from_f64(self.config.max_drawdown_pct).unwrap_or(dec!(0.15));
        if self.current_drawdown() > max_drawdown {
            warn!("📉 Max drawdown alcanzado: {:.2}%", self.current_drawdown() * dec!(100));
            if self.config.kill_switch_enabled {
                self.engage_kill_switch();
            }
            return true;
        }
        false
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
        tracing::error!(
            "🚨 KILL SWITCH ACTIVADO - Protegiendo capital restante: ${:.2}",
            self.current_balance
        );
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

    pub fn max_position_usd(&self, exchange: &str, symbol: &str) -> Decimal {
        self.get_max_position_for_symbol(exchange, symbol)
    }

    pub fn max_daily_loss_usd(&self) -> Decimal {
        self.get_max_daily_loss_usd()
    }
}
