//! Gestor de mercado

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use rust_decimal::prelude::{FromPrimitive, ToPrimitive};
use tracing::{info, warn, error, debug};
use std::sync::Arc;
use rand::Rng;
use uuid::Uuid;

use crate::types::{
    L2Book, BookLevel, OrderRequest, OrderSide,
    MarketConfig, Position
};
use crate::strategy::{AvellanedaStoikov, QuotedPrices};
use crate::risk::RiskManager;
use crate::executor::OrderExecutor;
use crate::notifier::TelegramNotifier;

#[derive(Debug, Clone, PartialEq)]
enum MarketState {
    Initializing,
    Running,
    Paused,
    Stopped,
}

#[derive(Clone)]
pub struct MarketManager {
    config: MarketConfig,
    pub symbol: String,
    state: MarketState,
    enabled: bool,
    book: Option<L2Book>,
    strategy: AvellanedaStoikov,
    risk_manager: Arc<tokio::sync::Mutex<RiskManager>>,
    executor: Arc<OrderExecutor>,
    notifier: Arc<TelegramNotifier>,
    current_orders: Vec<String>,
    last_refresh: chrono::DateTime<chrono::Utc>,
    current_price: f64,
    last_fill_check: chrono::DateTime<chrono::Utc>,
    _position: Option<Position>,
    _price_history: Vec<Decimal>,
    is_mainnet: bool,
    is_bybit: bool,
}

impl MarketManager {
    pub fn new(
        config: MarketConfig,
        strategy: AvellanedaStoikov,
        risk_manager: Arc<tokio::sync::Mutex<RiskManager>>,
        executor: Arc<OrderExecutor>,
        notifier: Arc<TelegramNotifier>,
        is_mainnet: bool,
        is_bybit: bool,
    ) -> Self {
        let initial_price = match config.symbol.as_str() {
            "BTC" => 50000.0,
            "ETH" => 3000.0,
            "BNB" => 600.0,
            "SOL" => 150.0,
            "XRP" => 0.50,
            _ => 100.0,
        };
        Self {
            symbol: config.symbol.clone(),
            config: config.clone(),
            state: MarketState::Initializing,
            book: None,
            strategy,
            risk_manager,
            executor,
            notifier,
            current_orders: Vec::new(),
            last_refresh: chrono::Utc::now(),
            current_price: initial_price,
            last_fill_check: chrono::Utc::now(),
            _position: None,
            _price_history: Vec::with_capacity(100),
            is_mainnet,
            is_bybit,
            enabled: config.enabled,
        }
    }

    pub fn set_exchange(&mut self, exchange: &str) {
        let was_mainnet = self.is_mainnet;
        let was_bybit = self.is_bybit;
        
        self.is_mainnet = exchange == "hyperliquid_mainnet";
        self.is_bybit = exchange == "bybit";
        
        if self.is_mainnet && !was_mainnet {
            info!("🔄 Mercado {} conectado a Hyperliquid Mainnet", self.symbol);
        } else if self.is_bybit && !was_bybit {
            info!("🔄 Mercado {} conectado a Bybit", self.symbol);
        } else if !self.is_mainnet && !self.is_bybit && (was_mainnet || was_bybit) {
            info!("🔄 Mercado {} cambiado a modo simulado", self.symbol);
        }
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        if !enabled && self.enabled {
            info!("🛑 Mercado {} deshabilitado", self.symbol);
        } else if enabled && !self.enabled {
            info!("✅ Mercado {} habilitado", self.symbol);
        }
        self.enabled = enabled;
    }
    
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub async fn start(&mut self) -> anyhow::Result<()> {
        info!("Iniciando market maker para {} ({})", 
            self.symbol, 
            if self.is_mainnet { "MAINNET" } else { "micro-capital" });
        
        if self.is_mainnet {
            match self.executor.get_mid_price(&self.symbol).await {
                Ok(Some(price)) => {
                    self.current_price = price.to_f64().unwrap_or(self.current_price);
                    info!("📊 Precio real obtido para {}: ${:.2}", self.symbol, self.current_price);
                }
                Ok(None) => {
                    info!("⚠️ Usando precio por defecto para {}", self.symbol);
                }
                Err(e) => {
                    warn!("⚠️ Error obteniendo precio real: {}, usando fallback", e);
                }
            }
        }
        
        self.state = MarketState::Running;
        
        if self.is_mainnet {
            if let Ok(Some(price)) = self.executor.get_mid_price(&self.symbol).await {
                let book = self.create_book_from_price(price.to_f64().unwrap());
                self.on_book_update(book).await;
                return Ok(());
            }
        }
        
        let initial_book = self.create_mock_book(self.current_price);
        if self.is_mainnet {
            info!("📊 Libro simulado como fallback para {} (${:.2})", self.symbol, self.current_price);
        } else {
            info!("📊 Libro simulado creado para {} (${:.2})", self.symbol, self.current_price);
        }
        self.on_book_update(initial_book).await;
        Ok(())
    }

    fn create_book_from_price(&self, price: f64) -> L2Book {
        let spread_pct = 0.002;
        let half_spread = price * spread_pct / 2.0;
        let bid_price_f64 = price - half_spread;
        let ask_price_f64 = price + half_spread;
        let bid_price = Decimal::from_f64(bid_price_f64).unwrap_or(dec!(50000));
        let ask_price = Decimal::from_f64(ask_price_f64).unwrap_or(dec!(50100));
        let base_size = match self.symbol.as_str() {
            "BTC" => 0.015,
            "ETH" => 0.15,
            "BNB" => 0.8,
            "SOL" => 3.0,
            "XRP" => 100.0,
            _ => 1.0,
        };
        L2Book {
            coin: self.symbol.clone(),
            time: chrono::Utc::now().timestamp_millis() as u64,
            bids: vec![
                BookLevel { px: bid_price, sz: Decimal::from_f64(base_size * 0.1).unwrap(), n: 5 },
                BookLevel { px: bid_price * dec!(0.998), sz: Decimal::from_f64(base_size * 0.15).unwrap(), n: 3 },
            ],
            asks: vec![
                BookLevel { px: ask_price, sz: Decimal::from_f64(base_size * 0.08).unwrap(), n: 4 },
                BookLevel { px: ask_price * dec!(1.002), sz: Decimal::from_f64(base_size * 0.12).unwrap(), n: 2 },
            ],
        }
    }

    fn create_mock_book(&self, base_price: f64) -> L2Book {
        let spread_pct = 0.002;
        let half_spread = base_price * spread_pct / 2.0;
        let bid_price_f64 = base_price - half_spread;
        let ask_price_f64 = base_price + half_spread;
        let bid_price = Decimal::from_f64(bid_price_f64).unwrap_or(dec!(50000));
        let ask_price = Decimal::from_f64(ask_price_f64).unwrap_or(dec!(50100));
        let base_size = match self.symbol.as_str() {
            "BTC" => 0.015,
            "ETH" => 0.15,
            "BNB" => 0.8,
            "SOL" => 3.0,
            "XRP" => 100.0,
            _ => 1.0,
        };
        L2Book {
            coin: self.symbol.clone(),
            time: chrono::Utc::now().timestamp_millis() as u64,
            bids: vec![
                BookLevel { px: bid_price, sz: Decimal::from_f64(base_size * 0.1).unwrap(), n: 5 },
                BookLevel { px: bid_price * dec!(0.998), sz: Decimal::from_f64(base_size * 0.15).unwrap(), n: 3 },
            ],
            asks: vec![
                BookLevel { px: ask_price, sz: Decimal::from_f64(base_size * 0.08).unwrap(), n: 4 },
                BookLevel { px: ask_price * dec!(1.002), sz: Decimal::from_f64(base_size * 0.12).unwrap(), n: 2 },
            ],
        }
    }

    pub async fn update_market_data(&mut self) {
        if self.state != MarketState::Running {
            return;
        }

        if !self.enabled {
            return;
        }

        if self.is_mainnet {
            match self.executor.get_mid_price(&self.symbol).await {
                Ok(Some(price)) => {
                    self.current_price = price.to_f64().unwrap_or(self.current_price);
                    let book = self.create_book_from_price(self.current_price);
                    self.on_book_update(book).await;
                    
                    let risk_manager = self.risk_manager.clone();
                    let symbol = self.symbol.clone();
                    tokio::spawn(async move {
                        let mut rm = risk_manager.lock().await;
                        rm.update_position_price(&symbol, price);
                    });
                    return;
                }
                Ok(None) => {
                    warn!("No price available for {}", self.symbol);
                }
                Err(e) => {
                    warn!("Error fetching price for {}: {}", self.symbol, e);
                }
            }
        }

        let mut rng = rand::thread_rng();
        let change_pct = (rng.gen::<f64>() - 0.5) * 0.005;
        let new_price = self.current_price * (1.0 + change_pct);
        let new_price = match self.symbol.as_str() {
            "BTC" => new_price.clamp(45000.0, 55000.0),
            "ETH" => new_price.clamp(2500.0, 3500.0),
            "BNB" => new_price.clamp(500.0, 700.0),
            "SOL" => new_price.clamp(120.0, 180.0),
            "XRP" => new_price.clamp(0.40, 0.60),
            _ => new_price,
        };
        self.current_price = new_price;
        let new_book = self.create_mock_book(new_price);
        self.on_book_update(new_book).await;
        if let Some(price) = Decimal::from_f64(new_price) {
            let risk_manager = self.risk_manager.clone();
            let symbol = self.symbol.clone();
            tokio::spawn(async move {
                let mut rm = risk_manager.lock().await;
                rm.update_position_price(&symbol, price);
            });
        }
        
        if !self.is_mainnet {
            let now = chrono::Utc::now();
            if (now - self.last_fill_check).num_seconds() > 20 {
                self.simulate_random_fill().await;
                self.last_fill_check = now;
            }
        }
    }

    async fn simulate_random_fill(&self) {
        let mut rng = rand::thread_rng();
        let balance = {
            let rm = self.risk_manager.lock().await;
            rm.current_balance().to_f64().unwrap_or(10.0)
        };
        if balance <= 0.10 {
            return;
        }
        if rng.gen::<f64>() < 0.4 {
            let side = if rng.gen_bool(0.5) { OrderSide::Buy } else { OrderSide::Sell };
            let max_trade_value = balance * 0.15;
            let size = match self.symbol.as_str() {
                "BTC" => Decimal::from_f64(0.00001 + rng.gen::<f64>() * (max_trade_value / 50000.0).min(0.00005)).unwrap(),
                "ETH" => Decimal::from_f64(0.0001 + rng.gen::<f64>() * (max_trade_value / 3000.0).min(0.0005)).unwrap(),
                "BNB" => Decimal::from_f64(0.001 + rng.gen::<f64>() * (max_trade_value / 600.0).min(0.005)).unwrap(),
                "SOL" => Decimal::from_f64(0.01 + rng.gen::<f64>() * (max_trade_value / 150.0).min(0.05)).unwrap(),
                "XRP" => Decimal::from_f64(0.5 + rng.gen::<f64>() * (max_trade_value / 0.5).min(10.0)).unwrap(),
                _ => Decimal::from_f64(0.1).unwrap(),
            };
            let max_offset_pct = 0.01;
            let price_offset = if side == OrderSide::Buy {
                - (self.current_price * rng.gen::<f64>() * max_offset_pct)
            } else {
                self.current_price * rng.gen::<f64>() * max_offset_pct
            };
            let price_f64 = (self.current_price + price_offset).max(0.01);
            let price = Decimal::from_f64(price_f64).unwrap();
            let trade_value = (size * price).to_f64().unwrap_or(0.0);
            if trade_value > max_trade_value * 1.1 {
                return;
            }
            let fill_id = format!("fill_{}_{}", self.symbol, Uuid::new_v4());
            info!("✅ FILL CONTROLADO: {} de {} por ${:.2} [ID: {}]", side, self.symbol, trade_value, fill_id);
            let notifier = self.notifier.clone();
            let symbol = self.symbol.clone();
            let side_str = format!("{}", side);
            let size_f64 = size.to_f64().unwrap_or(0.0);
            let price_f64 = price.to_f64().unwrap_or(0.0);
            let risk_manager = self.risk_manager.clone();
            let adjusted_size = if side == OrderSide::Buy { size } else { -size };
            tokio::spawn(async move {
                let mut rm = risk_manager.lock().await;
                let old_balance = rm.current_balance().to_f64().unwrap_or(0.0);
                rm.update_position(&symbol, adjusted_size, price, fill_id.clone());
                let new_balance = rm.current_balance().to_f64().unwrap_or(0.0);
                let pnl = new_balance - old_balance;
                if pnl.abs() > 0.01 && notifier.should_notify_fill().await {
                    let msg = notifier.format_fill(&symbol, &side_str, size_f64, price_f64, pnl);
                    if let Err(e) = notifier.send_message(&msg).await {
                        tracing::warn!("Error enviando notificación de fill: {}", e);
                    }
                }
                notifier.send_fill_to_dashboard(&symbol, &side_str, size_f64, price_f64, pnl, &fill_id).await;
                debug!("📤 Fill enviado - ID: {}, P&L: ${:.2}", fill_id, pnl);
            });
        }
    }

    pub async fn on_book_update(&mut self, book: L2Book) {
        if self.state != MarketState::Running {
            return;
        }
        self.strategy.update(&book);
        self.book = Some(book);
        if self.strategy.should_refresh(self.last_refresh) {
            if let Err(e) = self.refresh_orders().await {
                error!("Error refrescando órdenes para {}: {}", self.symbol, e);
            }
        }
    }

    async fn refresh_orders(&mut self) -> anyhow::Result<()> {
        let book = match &self.book {
            Some(b) => b,
            None => return Ok(()),
        };
        let inventory = Decimal::ZERO;
        let risk_manager = self.risk_manager.lock().await;
        let balance = risk_manager.current_balance();
        if balance < Decimal::from_f64(0.5).unwrap() {
            drop(risk_manager);
            return Ok(());
        }
        let max_inventory_value = balance * Decimal::from_f64(self.config.margin_alloc).unwrap_or(dec!(0.2));
        let max_inventory = if let (Some(bid), Some(ask)) = (book.bids.first(), book.asks.first()) {
            let mid = (bid.px + ask.px) / dec!(2);
            if mid > Decimal::ZERO {
                (max_inventory_value / mid).max(dec!(0.00001))
            } else {
                dec!(0.0001)
            }
        } else {
            dec!(0.0001)
        };
        drop(risk_manager);
        let quotes = self.strategy.calculate_quotes(inventory, max_inventory);
        if quotes.bid <= Decimal::ZERO || quotes.ask <= Decimal::ZERO || quotes.ask <= quotes.bid {
            return Ok(());
        }
        
        if !self.enabled {
            info!("⏸️ Mercado {} deshabilitado, cancelando órdenes", self.symbol);
            self.cancel_all_orders().await?;
            return Ok(());
        }
        
        self.cancel_all_orders().await?;
        self.place_orders(quotes.clone()).await?;
        self.last_refresh = chrono::Utc::now();
        let spread_pct = ((quotes.ask - quotes.bid) / quotes.bid * dec!(100)).to_f64().unwrap_or(0.0);
        info!("🔄 Órdenes para {}: bid=${:.2}, ask=${:.2} (spread: {:.2}%)", self.symbol, quotes.bid, quotes.ask, spread_pct);
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

    async fn place_orders(&mut self, quotes: QuotedPrices) -> anyhow::Result<()> {
        let risk_manager = self.risk_manager.lock().await;
        let max_position_usd = risk_manager.max_position_usd();
        drop(risk_manager);
        let max_order_usd = (max_position_usd * Decimal::from_f64(0.30).unwrap_or(dec!(0.30))).max(Decimal::from_f64(0.50).unwrap_or(dec!(0.50)));
        let current_price = Decimal::from_f64(self.current_price).unwrap_or(dec!(1));
        let base_size_usd = match self.symbol.as_str() {
            "BTC" => Decimal::from_f64(1.5).unwrap_or(dec!(1.5)),
            "ETH" => Decimal::from_f64(1.2).unwrap_or(dec!(1.2)),
            "BNB" => Decimal::from_f64(1.0).unwrap_or(dec!(1.0)),
            "SOL" => Decimal::from_f64(0.75).unwrap_or(dec!(0.75)),
            "XRP" => Decimal::from_f64(0.50).unwrap_or(dec!(0.50)),
            _ => Decimal::from_f64(0.50).unwrap_or(dec!(0.50)),
        };
        let order_usd = base_size_usd.min(max_order_usd);
        let order_usd = order_usd.max(dec!(10));
        let mut base_size = order_usd / current_price;
        let min_size = Decimal::from_f64(self.config.min_size).unwrap_or(dec!(0.00001));
        base_size = base_size.max(min_size);
        
        if self.is_mainnet {
            base_size = self.executor.round_size(&self.symbol, base_size).unwrap_or(base_size);
        }
        
        info!("📊 Tamaño de orden para {}: ${:.2} USD ({:.6} unidades, límite máx: ${:.2})", self.symbol, order_usd, base_size, max_position_usd);
        if quotes.bid > Decimal::ZERO {
            let bid_price = if self.is_mainnet {
                self.executor.round_price(&self.symbol, quotes.bid).unwrap_or(quotes.bid)
            } else {
                quotes.bid
            };
            let buy_order = OrderRequest::limit(&self.symbol, self.config.asset_index, OrderSide::Buy, base_size, bid_price);
            let risk_check = {
                let mut rm = self.risk_manager.lock().await;
                rm.check_order(&buy_order, quotes.bid)
            };
            if risk_check {
                if let Ok(order_id) = self.executor.place_order(buy_order.clone()).await {
                    info!("💰 Orden COMPRA: {} ${:.2}", self.symbol, (buy_order.quantity * buy_order.price.unwrap()).to_f64().unwrap_or(0.0));
                    self.current_orders.push(order_id);
                } else {
                    warn!("❌ Error colocando orden COMPRA para {}", self.symbol);
                }
            }
        }
        if quotes.ask > Decimal::ZERO {
            let ask_price = if self.is_mainnet {
                self.executor.round_price(&self.symbol, quotes.ask).unwrap_or(quotes.ask)
            } else {
                quotes.ask
            };
            let sell_order = OrderRequest::limit(&self.symbol, self.config.asset_index, OrderSide::Sell, base_size, ask_price);
            let risk_check = {
                let mut rm = self.risk_manager.lock().await;
                rm.check_order(&sell_order, quotes.ask)
            };
            if risk_check {
                if let Ok(order_id) = self.executor.place_order(sell_order.clone()).await {
                    info!("💰 Orden VENTA: {} ${:.2}", self.symbol, (sell_order.quantity * sell_order.price.unwrap()).to_f64().unwrap_or(0.0));
                    self.current_orders.push(order_id);
                } else {
                    warn!("❌ Error colocando orden VENTA para {}", self.symbol);
                }
            }
        }
        Ok(())
    }

    pub async fn on_order_fill(&mut self, order_id: &str, size: Decimal, price: Decimal) {
        info!("✅ Fill en {}: ${:.2}", self.symbol, (size * price).to_f64().unwrap_or(0.0));
        {
            let mut risk_manager = self.risk_manager.lock().await;
            risk_manager.update_position(&self.symbol, size, price, order_id.to_string());
        }
        self.current_orders.retain(|id| id != order_id);
    }

    pub async fn stop(&mut self) -> anyhow::Result<()> {
        info!("Deteniendo market maker para {}", self.symbol);
        self.state = MarketState::Stopped;
        self.cancel_all_orders().await?;
        Ok(())
    }
}