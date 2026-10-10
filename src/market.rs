//! Gestor de mercado REAL (conecta a Hyperliquid)
//!
//! Ciclo (cada ~10 s, desde bot.rs):
//!   1. Lee mid, posición real y equity del exchange.
//!   2. Si el risk manager o el stop por posición lo exigen, cancela todo y cierra.
//!   3. Si hay posición abierta, cotiza la salida con una orden reduce-only.
//!   4. Re-cotiza solo cuando cambian los precios redondeados o la posición
//!      (ahorra rate-limit de Hyperliquid).

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use rust_decimal::prelude::{FromPrimitive, ToPrimitive};
use tracing::{info, warn, error, debug};
use std::sync::Arc;

use crate::types::{L2Book, OrderRequest, OrderSide, MarketConfig};
use crate::strategy::{AvellanedaStoikov, QuotedPrices, MAKER_FEE};
use crate::risk::RiskManager;
use crate::executor::OrderExecutor;
use crate::notifier::TelegramNotifier;
use crate::hyperliquid::HyperliquidClient;
use crate::executor::ExchangeType;

/// Valor mínimo de orden en Hyperliquid
const MIN_ORDER_USD: Decimal = dec!(10);
/// Movimiento adverso desde el precio de entrada que dispara el cierre a mercado
const POSITION_STOP_LOSS_PCT: Decimal = dec!(0.01);
/// Slippage máximo aceptado al cerrar a mercado
const CLOSE_SLIPPAGE: Decimal = dec!(0.01);
/// Cerrar la posición abierta al detener el bot
const CLOSE_POSITION_ON_STOP: bool = true;
/// Tiempo máximo con una posición abierta antes de cerrarla a mercado.
/// En el backtest, no quedarse "casado" con una posición fue clave para limitar pérdidas.
const MAX_HOLD_MINUTES: i64 = 30;
/// Apalancamiento EFECTIVO máximo (nocional total / equity real), independiente de la
/// configuración. El backtest mostró que el apalancamiento multiplica las pérdidas igual
/// que las ganancias; subirlo solo tiene sentido con ventaja demostrada en real.
const MAX_EFFECTIVE_LEVERAGE: Decimal = dec!(2);

#[derive(Debug, Clone, PartialEq)]
enum MarketState {
    Initializing,
    Running,
    Stopped,
}

/// Orden en el libro creada por este bot
#[derive(Debug, Clone)]
struct RestingOrder {
    id: String,
    side: OrderSide,
}

#[derive(Clone)]
pub struct MarketManager {
    config: MarketConfig,
    pub symbol: String,
    state: MarketState,
    client: Arc<HyperliquidClient>,
    strategy: AvellanedaStoikov,
    risk_manager: Arc<tokio::sync::Mutex<RiskManager>>,
    executor: Arc<OrderExecutor>,
    notifier: Arc<TelegramNotifier>,
    current_orders: Vec<RestingOrder>,
    last_refresh: chrono::DateTime<chrono::Utc>,
    last_price: Decimal,
    exchange: ExchangeType,
    /// Posición real (con signo) y precio de entrada
    position: Decimal,
    entry_price: Decimal,
    /// Momento en que se abrió la posición actual
    position_opened_at: Option<chrono::DateTime<chrono::Utc>>,
    /// Lo último que se cotizó: (bid, ask, posición) — para no re-cotizar sin cambios
    last_quoted: Option<(Option<Decimal>, Option<Decimal>, Decimal)>,
}

impl MarketManager {
    pub fn new(
        config: MarketConfig,
        client: Arc<HyperliquidClient>,
        strategy: AvellanedaStoikov,
        risk_manager: Arc<tokio::sync::Mutex<RiskManager>>,
        executor: Arc<OrderExecutor>,
        notifier: Arc<TelegramNotifier>,
        exchange: ExchangeType,
    ) -> Self {
        Self {
            symbol: config.symbol.clone(),
            config,
            state: MarketState::Initializing,
            client,
            strategy,
            risk_manager,
            executor,
            notifier,
            current_orders: Vec::new(),
            last_refresh: chrono::Utc::now(),
            last_price: Decimal::ZERO,
            exchange,
            position: Decimal::ZERO,
            entry_price: Decimal::ZERO,
            position_opened_at: None,
            last_quoted: None,
        }
    }

    pub async fn start(&mut self) -> anyhow::Result<()> {
        info!("Iniciando market maker para {}", self.symbol);

        // Limpiar órdenes huérfanas de ejecuciones anteriores
        match self.executor.cancel_all(&self.symbol).await {
            Ok(n) if n > 0 => warn!("🧹 {} órdenes huérfanas canceladas en {}", n, self.symbol),
            Ok(_) => {}
            Err(e) => warn!("No se pudieron limpiar órdenes de {}: {}", self.symbol, e),
        }

        self.state = MarketState::Running;
        self.update_market_data().await;
        Ok(())
    }

    /// Ciclo principal: se llama periódicamente desde bot.rs
    pub async fn update_market_data(&mut self) {
        if self.state != MarketState::Running {
            return;
        }

        match self.client.get_mid_price(&self.symbol).await {
            Ok(price) => {
                self.last_price = price;
                self.strategy.update_price(price);
            }
            Err(e) => {
                error!("Error obtener precio {}: {}", self.symbol, e);
                return;
            }
        }

        if let Err(e) = self.sync_with_exchange().await {
            // Sin conocer la posición real no se cotiza
            error!("Error sincronizando {} con el exchange: {}", self.symbol, e);
            return;
        }

        if self.check_exits().await {
            return;
        }

        if !self.strategy.is_warmed_up() {
            debug!("⏳ {}: acumulando precios para estimar volatilidad", self.symbol);
            return;
        }

        if let Err(e) = self.refresh_orders().await {
            error!("Error refrescando órdenes para {}: {}", self.symbol, e);
        }
    }

    /// Lee posición y equity reales del exchange
    async fn sync_with_exchange(&mut self) -> anyhow::Result<()> {
        if let Some((size, entry)) = self.executor.get_position_info(&self.symbol).await? {
            if size != self.position {
                info!(
                    "📊 Posición {}: {} → {} (entrada ${:.4})",
                    self.symbol, self.position, size, entry
                );
            }
            if size.is_zero() {
                self.position_opened_at = None;
            } else if self.position.is_zero() || self.position_opened_at.is_none() {
                self.position_opened_at = Some(chrono::Utc::now());
            }
            self.position = size;
            self.entry_price = entry;
        }

        if let Some(equity) = self.executor.get_account_value().await? {
            let mut rm = self.risk_manager.lock().await;
            rm.sync_balance(equity);
            rm.update_position_price(&self.symbol, self.last_price);
        }
        Ok(())
    }

    /// Cierra la posición si el riesgo lo exige. Devuelve `true` si no debe cotizarse.
    async fn check_exits(&mut self) -> bool {
        let flatten = self.risk_manager.lock().await.should_flatten();

        let mut position_stop = false;
        if !self.position.is_zero() && self.entry_price > Decimal::ZERO && self.last_price > Decimal::ZERO {
            let move_pct = (self.last_price - self.entry_price) / self.entry_price;
            let adverse = if self.position > Decimal::ZERO { -move_pct } else { move_pct };
            if adverse > POSITION_STOP_LOSS_PCT {
                warn!(
                    "🛑 Stop por posición en {}: movimiento adverso {:.2}% (entrada ${:.4}, mid ${:.4})",
                    self.symbol,
                    adverse * dec!(100),
                    self.entry_price,
                    self.last_price
                );
                position_stop = true;
            }
        }

        if let Some(opened) = self.position_opened_at {
            let held = (chrono::Utc::now() - opened).num_minutes();
            if !self.position.is_zero() && held >= MAX_HOLD_MINUTES {
                warn!("⏱️ Posición de {} abierta {} min: cerrando por tiempo", self.symbol, held);
                position_stop = true;
            }
        }

        if !flatten && !position_stop {
            return false;
        }

        self.cancel_all_orders().await;
        if !self.position.is_zero() {
            match self.executor.close_position(&self.symbol, CLOSE_SLIPPAGE).await {
                Ok(()) => {
                    let msg = format!("🔻 Posición de {} cerrada ({})", self.symbol, self.position);
                    info!("{}", msg);
                    let _ = self.notifier.send_message(&msg).await;
                }
                Err(e) => error!("❌ No se pudo cerrar posición de {}: {}", self.symbol, e),
            }
        }
        // Con el stop por posición se vuelve a cotizar en el próximo ciclo;
        // con el risk manager activado el bot queda sin abrir nuevas posiciones.
        true
    }

    pub async fn on_book_update(&mut self, book: L2Book) {
        if self.state != MarketState::Running {
            return;
        }
        if let (Some(bid), Some(ask)) = (book.bids.first(), book.asks.first()) {
            self.last_price = (bid.px + ask.px) / dec!(2);
        }
        self.strategy.update(&book);
    }

    async fn refresh_orders(&mut self) -> anyhow::Result<()> {
        let exchange_name = match self.exchange {
            ExchangeType::Simulated => "simulated",
            ExchangeType::HyperliquidMainnet => "hyperliquid",
            ExchangeType::Bybit => "bybit",
        };

        let (balance, max_position_usd) = {
            let rm = self.risk_manager.lock().await;
            (rm.current_balance(), rm.max_position_usd(exchange_name, &self.symbol))
        };
        let max_position_usd = max_position_usd.min(balance * MAX_EFFECTIVE_LEVERAGE);

        if balance < dec!(0.50) {
            return Ok(());
        }

        let mid = self.last_price;
        let max_inventory_usd = max_position_usd.max(MIN_ORDER_USD);
        let max_inventory = max_inventory_usd / mid;

        let quotes = match self.strategy.calculate_quotes(self.position, max_inventory) {
            Some(q) if q.bid > Decimal::ZERO && q.ask > q.bid => q,
            _ => return Ok(()),
        };

        let plan = self.plan_orders(&quotes, max_position_usd);

        // No re-cotizar si nada cambió (ahorra rate-limit y prioridad en el libro)
        let key = (
            plan.buy.as_ref().map(|(_, px, _)| *px),
            plan.sell.as_ref().map(|(_, px, _)| *px),
            self.position,
        );
        let unchanged = self.last_quoted == Some(key) && !self.current_orders.is_empty();
        if unchanged && !self.strategy.should_refresh(self.last_refresh) {
            return Ok(());
        }

        self.cancel_all_orders().await;

        if let Some((size, px, reduce_only)) = plan.buy {
            self.place(OrderSide::Buy, size, px, reduce_only).await;
        }
        if let Some((size, px, reduce_only)) = plan.sell {
            self.place(OrderSide::Sell, size, px, reduce_only).await;
        }

        self.last_quoted = Some(key);
        self.last_refresh = chrono::Utc::now();
        info!(
            "🔄 {} mid=${:.4} pos={} bid={:?} ask={:?} spread={:.3}%",
            self.symbol,
            mid,
            self.position,
            key.0,
            key.1,
            (quotes.spread_pct * dec!(100)).to_f64().unwrap_or(0.0)
        );
        Ok(())
    }

    /// Decide qué órdenes colocar: (tamaño, precio, reduce_only) por lado
    fn plan_orders(&self, quotes: &QuotedPrices, max_position_usd: Decimal) -> OrderPlan {
        let mid = self.last_price;
        let order_usd = (max_position_usd * dec!(0.25)).max(MIN_ORDER_USD);
        let min_size = Decimal::from_f64(self.config.min_size).unwrap_or(Decimal::ZERO);
        let base_size = (order_usd / mid).max(min_size);
        let position_usd = self.position.abs() * mid;
        let can_add = position_usd + order_usd <= max_position_usd.max(MIN_ORDER_USD);

        let bid = self.executor.round_price_dir(&self.symbol, quotes.bid, false);
        let ask = self.executor.round_price_dir(&self.symbol, quotes.ask, true);

        // Salida de una posición: nunca por debajo del break-even tras fees
        // (si el precio se va en contra, actúa el stop por posición)
        let breakeven_fee = MAKER_FEE * dec!(2);
        // El filtro de tendencia solo afecta a las órdenes que ABREN o aumentan posición
        let (allow_buy, allow_sell) = self.strategy.trend_filter();

        let mut plan = OrderPlan::default();
        if self.position > Decimal::ZERO {
            let exit = ask.max(self.executor.round_price_dir(
                &self.symbol,
                self.entry_price * (Decimal::ONE + breakeven_fee),
                true,
            ));
            plan.sell = Some((self.position.abs(), exit, true));
            if can_add && allow_buy {
                plan.buy = Some((base_size, bid, false));
            }
        } else if self.position < Decimal::ZERO {
            let exit = bid.min(self.executor.round_price_dir(
                &self.symbol,
                self.entry_price * (Decimal::ONE - breakeven_fee),
                false,
            ));
            plan.buy = Some((self.position.abs(), exit, true));
            if can_add && allow_sell {
                plan.sell = Some((base_size, ask, false));
            }
        } else {
            if allow_buy {
                plan.buy = Some((base_size, bid, false));
            }
            if allow_sell {
                plan.sell = Some((base_size, ask, false));
            }
        }
        plan
    }

    async fn place(&mut self, side: OrderSide, size: Decimal, price: Decimal, reduce_only: bool) {
        let mut order = OrderRequest::limit(&self.symbol, self.config.asset_index, side, size, price).post_only();
        if reduce_only {
            order = order.reduce_only();
        }

        let allowed = self.risk_manager.lock().await.check_order(&order, price);
        if !allowed {
            debug!("⛔ Orden {:?} rechazada por risk manager para {}", side, self.symbol);
            return;
        }

        match self.executor.place_order(order).await {
            Ok(order_id) => {
                info!(
                    "💰 {} {} {} @ ${:.4}{}",
                    side,
                    size,
                    self.symbol,
                    price,
                    if reduce_only { " (reduce-only)" } else { "" }
                );
                self.current_orders.push(RestingOrder { id: order_id, side });
            }
            Err(e) => warn!("❌ No se colocó orden {:?} en {}: {}", side, self.symbol, e),
        }
    }

    async fn cancel_all_orders(&mut self) {
        for order in &self.current_orders {
            if let Err(e) = self
                .executor
                .cancel_order(&self.symbol, self.config.asset_index, &order.id)
                .await
            {
                warn!("Error cancelando orden {:?} {}: {}", order.side, order.id, e);
            }
        }
        self.current_orders.clear();
    }

    pub async fn on_order_fill(&mut self, order_id: &str, size: Decimal, price: Decimal) {
        info!("✅ Fill REAL en {}: ${:.2}", self.symbol, (size * price).to_f64().unwrap_or(0.0));
        {
            let mut rm = self.risk_manager.lock().await;
            rm.update_position(&self.symbol, size, price, order_id.to_string());
        }
        self.current_orders.retain(|o| o.id != order_id);
    }

    pub async fn stop(&mut self) -> anyhow::Result<()> {
        info!("Deteniendo market maker REAL para {}", self.symbol);
        self.state = MarketState::Stopped;
        self.cancel_all_orders().await;
        if let Err(e) = self.executor.cancel_all(&self.symbol).await {
            warn!("Error cancelando órdenes restantes de {}: {}", self.symbol, e);
        }
        if CLOSE_POSITION_ON_STOP {
            if let Err(e) = self.executor.close_position(&self.symbol, CLOSE_SLIPPAGE).await {
                error!("❌ No se pudo cerrar posición de {} al detener: {}", self.symbol, e);
            }
        }
        Ok(())
    }
}

#[derive(Default)]
struct OrderPlan {
    buy: Option<(Decimal, Decimal, bool)>,
    sell: Option<(Decimal, Decimal, bool)>,
}
