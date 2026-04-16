//! Gestor de mercado REAL (conecta a Hyperliquid)

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use rust_decimal::prelude::{FromPrimitive, ToPrimitive};
use tracing::{info, warn, error, debug};
use std::sync::Arc;

use crate::types::{L2Book, OrderRequest, OrderSide, MarketConfig};
use crate::strategy::{AvellanedaStoikov, QuotedPrices};
use crate::risk::RiskManager;
use crate::executor::OrderExecutor;
use crate::notifier::TelegramNotifier;
use crate::hyperliquid::HyperliquidClient;
use crate::executor::ExchangeType;

#[derive(Debug, Clone, PartialEq)]
enum MarketState {
    Initializing,
    Running,
    Stopped,
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
    current_orders: Vec<String>,
    last_refresh: chrono::DateTime<chrono::Utc>,
    last_price: Decimal,
    exchange: ExchangeType,
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
        }
    }

    pub async fn start(&mut self) -> anyhow::Result<()> {
        info!("Iniciando market maker para {}", self.symbol);
        self.state = MarketState::Running;
        
        match self.client.get_mid_price(&self.symbol).await {
            Ok(price) => {
                self.last_price = price;
                self.strategy.update_price(price);
                info!("📊 Precio {}: ${:.2}", self.symbol, price);
            }
            Err(e) => {
                warn!("No precio {}: {}", self.symbol, e);
                self.last_price = dec!(50000);
            }
        }
        
        self.refresh_orders().await?;
        Ok(())
    }

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
            }
        }
        
        if let Ok(state) = self.client.get_account_state().await {
            if let Some(&position) = state.balances.get(&self.symbol) {
                if position != Decimal::ZERO {
                    info!("📊 Posición {} en Hyperliquid: {}", self.symbol, position);
                }
            }
        }
    }

    pub async fn on_book_update(&mut self, book: L2Book) {
        if self.state != MarketState::Running {
            return;
        }

        // Actualizar precio medio
        if let (Some(bid), Some(ask)) = (book.bids.first(), book.asks.first()) {
            self.last_price = (bid.px + ask.px) / dec!(2);
        }

        self.strategy.update(&book);

        if self.strategy.should_refresh(self.last_refresh) {
            if let Err(e) = self.refresh_orders().await {
                error!("Error refrescando órdenes para {}: {}", self.symbol, e);
            }
        }
    }

    async fn refresh_orders(&mut self) -> anyhow::Result<()> {
        let inventory = Decimal::ZERO;

        let risk_manager = self.risk_manager.lock().await;
        let balance = risk_manager.current_balance();
        
        let exchange_name = match self.exchange {
            ExchangeType::Simulated => "simulated",
            ExchangeType::HyperliquidMainnet => "hyperliquid",
            ExchangeType::Bybit => "bybit",
        };
        
        // Usar 50% del máximo por mercado (el riesgo ya incluye leverage)
        let max_position_usd = risk_manager.max_position_usd(exchange_name, &self.symbol) * Decimal::from_f64(0.5).unwrap_or(dec!(0.5));
        
        if balance < Decimal::from_f64(0.50).unwrap() {
            drop(risk_manager);
            return Ok(());
        }

        let max_inventory_value = balance * Decimal::from_f64(self.config.margin_alloc).unwrap_or(dec!(0.2));
        let mid = self.last_price;
        let max_inventory = if mid > Decimal::ZERO {
            (max_inventory_value / mid).max(dec!(0.00001))
        } else {
            dec!(0.0001)
        };
        drop(risk_manager);

        let quotes = self.strategy.calculate_quotes(inventory, max_inventory);
        if quotes.bid <= Decimal::ZERO || quotes.ask <= Decimal::ZERO || quotes.ask <= quotes.bid {
            return Ok(());
        }

        let spread_pct = ((quotes.ask - quotes.bid) / quotes.bid * dec!(100)).to_f64().unwrap_or(0.0);
        
        // Cancelar órdenes existentes
        self.cancel_all_orders().await?;

        // Colocar nuevas órdenes
        self.place_orders(quotes.clone(), max_position_usd).await?;

        self.last_refresh = chrono::Utc::now();
        info!("🔄 Órdenes REALES para {}: bid=${:.2}, ask=${:.2} (spread: {:.2}%)", 
              self.symbol, quotes.bid, quotes.ask, spread_pct);
        Ok(())
    }

    async fn cancel_all_orders(&mut self) -> anyhow::Result<()> {
        for order_id in &self.current_orders {
            if let Err(e) = self.executor.cancel_order(order_id).await {
                warn!("Error cancelando orden {}: {}", order_id, e);
            }
        }
        self.current_orders.clear();
        Ok(())
    }

    async fn place_orders(&mut self, quotes: QuotedPrices, max_position_usd: Decimal) -> anyhow::Result<()> {
        const MIN_ORDER_VALUE: f64 = 10.0;
        const MAX_ORDER_PCT: f64 = 0.25;
        
        let max_order_usd = (max_position_usd * Decimal::from_f64(MAX_ORDER_PCT).unwrap_or(dec!(0.25)))
            .max(Decimal::from_f64(MIN_ORDER_VALUE).unwrap_or(dec!(10)));
        
        // Calcular tamaño base en USD (limitado por max_order_usd)
        let order_usd = max_order_usd;
        let base_size = order_usd / self.last_price;
        
        let min_size = Decimal::from_f64(self.config.min_size).unwrap_or(dec!(0.00001));
        let base_size = base_size.max(min_size);

        info!("📊 Tamaño de orden para {}: ${:.2} USD ({:.6} unidades)", 
              self.symbol, order_usd, base_size);

        // Orden de compra
        if quotes.bid > Decimal::ZERO {
            let buy_order = OrderRequest::limit(&self.symbol, self.config.asset_index, OrderSide::Buy, base_size, quotes.bid);
            let risk_check = {
                let mut rm = self.risk_manager.lock().await;
                rm.check_order(&buy_order, quotes.bid)
            };
            if risk_check {
                match self.executor.place_order(buy_order).await {
                    Ok(order_id) => {
                        info!("💰 Orden COMPRA REAL: {} ${:.2}", self.symbol, order_usd);
                        self.current_orders.push(order_id);
                    }
                    Err(e) => error!("❌ Error colocando orden COMPRA para {}: {}", self.symbol, e),
                }
            } else {
                debug!("⛔ Orden COMPRA rechazada por risk manager para {}", self.symbol);
            }
        }

        // Orden de venta
        if quotes.ask > Decimal::ZERO {
            let sell_order = OrderRequest::limit(&self.symbol, self.config.asset_index, OrderSide::Sell, base_size, quotes.ask);
            let risk_check = {
                let mut rm = self.risk_manager.lock().await;
                rm.check_order(&sell_order, quotes.ask)
            };
            if risk_check {
                match self.executor.place_order(sell_order).await {
                    Ok(order_id) => {
                        info!("💰 Orden VENTA REAL: {} ${:.2}", self.symbol, order_usd);
                        self.current_orders.push(order_id);
                    }
                    Err(e) => error!("❌ Error colocando orden VENTA para {}: {}", self.symbol, e),
                }
            } else {
                debug!("⛔ Orden VENTA rechazada por risk manager para {}", self.symbol);
            }
        }

        Ok(())
    }

    pub async fn on_order_fill(&mut self, order_id: &str, size: Decimal, price: Decimal) {
        info!("✅ Fill REAL en {}: ${:.2}", self.symbol, (size * price).to_f64().unwrap_or(0.0));
        {
            let mut rm = self.risk_manager.lock().await;
            rm.update_position(&self.symbol, size, price, order_id.to_string());
        }
        self.current_orders.retain(|id| id != order_id);
    }

    pub async fn stop(&mut self) -> anyhow::Result<()> {
        info!("Deteniendo market maker REAL para {}", self.symbol);
        self.state = MarketState::Stopped;
        self.cancel_all_orders().await?;
        Ok(())
    }
}