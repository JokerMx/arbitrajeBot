//! Orquestador principal del bot

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use rust_decimal::prelude::ToPrimitive;
use tracing::{info, error};
use tokio::sync::Mutex;
use std::sync::Arc;
use std::collections::HashMap;
use tokio::signal;
use tokio::time;

use crate::types::{MarketConfig, StrategyConfig, RiskConfig};
use crate::market::MarketManager;
use crate::strategy::AvellanedaStoikov;
use crate::risk::RiskManager;
use crate::executor::OrderExecutor;
use crate::notifier::TelegramNotifier;
use crate::executor::ExchangeType;

#[derive(Clone)]
pub struct TradingBot {
    markets: HashMap<String, MarketManager>,
    risk_manager: Arc<Mutex<RiskManager>>,
    executor: Arc<OrderExecutor>,
    notifier: Arc<TelegramNotifier>,
    _config: BotConfig,
}

#[derive(Debug, Clone)]
pub struct BotConfig {
    pub private_key: String,
    pub wallet: String,
    pub network: String,
    pub log_level: String,
    pub status_notify_interval_secs: u64,
}

impl BotConfig {
    pub fn new(private_key: String, wallet: String, network: String, log_level: String) -> Self {
        Self {
            private_key,
            wallet,
            network,
            log_level,
            status_notify_interval_secs: 60,
        }
    }
}

impl TradingBot {
    pub async fn new(
        config: BotConfig,
        market_configs: Vec<MarketConfig>,
        strategy_config: StrategyConfig,
        _risk_config: RiskConfig,
        _initial_capital: Decimal,
        notifier: Arc<TelegramNotifier>,
        risk_manager: Arc<Mutex<RiskManager>>,
        exchange_type: ExchangeType,
    ) -> anyhow::Result<Self> {
        info!("Inicializando TradingBot en red: {}", config.network);
        
        let is_testnet = config.network == "testnet";
        let executor = Arc::new(OrderExecutor::new(
            &config.private_key,
            is_testnet,
            exchange_type,
        ).await?);
        
        let mut markets = HashMap::new();
        for market_config in market_configs {
            if !market_config.enabled {
                continue;
            }
            
            info!("Configurando market maker para {}", market_config.symbol);
            
            let strategy = AvellanedaStoikov::new(strategy_config.clone());
            let market = MarketManager::new(
                market_config.clone(),
                strategy,
                risk_manager.clone(),
                executor.clone(),
                notifier.clone(),
            );
            
            markets.insert(market_config.symbol.clone(), market);
        }
        
        Ok(Self {
            markets,
            risk_manager,
            executor,
            notifier,
            _config: config,
        })
    }

    pub async fn start(&mut self) -> anyhow::Result<()> {
        info!("🚀 Iniciando bot de market making");
        
        for (symbol, market) in self.markets.iter_mut() {
            if let Err(e) = market.start().await {
                error!("Error iniciando mercado {}: {}", symbol, e);
            }
        }
        
        Ok(())
    }

    pub async fn update_markets(&mut self) {
        for market in self.markets.values_mut() {
            market.update_market_data().await;
        }
    }

    pub async fn stop(&mut self) -> anyhow::Result<()> {
        info!("🛑 Deteniendo bot...");
        
        for (symbol, market) in self.markets.iter_mut() {
            if let Err(e) = market.stop().await {
                error!("Error deteniendo mercado {}: {}", symbol, e);
            }
        }
        
        self.executor.shutdown().await?;
        
        let risk = self.risk_manager.lock().await;
        info!("📊 === ESTADÍSTICAS FINALES ===");
        info!("💰 Balance final: ${:.2}", risk.current_balance());
        info!("📈 P&L Total: ${:.2}", risk.total_pnl());
        info!("🎯 Win Rate: {:.1}%", risk.win_rate() * 100.0);
        info!("📊 Volumen total: ${:.2}", risk.total_volume());
        info!("🏆 Trades ganadores: {}", risk.winning_trades());
        info!("💔 Trades perdedores: {}", risk.losing_trades());
        drop(risk);
        
        info!("✅ Bot detenido correctamente");
        Ok(())
    }

    async fn monitoring_loop(risk_manager: Arc<Mutex<RiskManager>>, notifier: Arc<TelegramNotifier>) {
        let mut interval = time::interval(time::Duration::from_secs(30));
        
        loop {
            interval.tick().await;
            
            let risk = risk_manager.lock().await;
            let balance = risk.current_balance().to_f64().unwrap_or(0.0);
            let pnl = risk.total_pnl().to_f64().unwrap_or(0.0);
            let win_rate = risk.win_rate();
            let drawdown = risk.current_drawdown().to_f64().unwrap_or(0.0);
            
            info!("📊 === ESTADO DEL BOT ===");
            info!("💰 Balance: ${:.2}", risk.current_balance());
            info!("📈 P&L: ${:.2}", risk.total_pnl());
            info!("📉 Drawdown: {:.2}%", risk.current_drawdown() * dec!(100));
            info!("🎯 Win Rate: {:.1}%", risk.win_rate() * 100.0);
            info!("📊 Volumen: ${:.2}", risk.total_volume());
            info!("⚠️ Estado riesgo: {:?}", risk.status());
            
            let recent = risk.recent_trades(3);
            if !recent.is_empty() {
                info!("🔄 Últimos trades:");
                for trade in recent {
                    info!("   {} {} {} @ ${:.2} (P&L: ${:.2})", 
                          trade.side, trade.size, trade.symbol, trade.price, trade.pnl);
                }
            }
            
            drop(risk);
            
            if notifier.should_notify_status().await {
                let status_msg = notifier.format_status(balance, pnl, win_rate, drawdown);
                let _ = notifier.send_message(&status_msg).await;
            }
        }
    }

    pub async fn run_until_signal(mut self) -> anyhow::Result<()> {
        self.start().await?;
        
        let risk_manager_clone = self.risk_manager.clone();
        let notifier_clone = self.notifier.clone();
        tokio::spawn(async move {
            Self::monitoring_loop(risk_manager_clone, notifier_clone).await;
        });
        
        let mut update_interval = time::interval(time::Duration::from_secs(10));
        
        let ctrl_c = signal::ctrl_c();
        tokio::pin!(ctrl_c);
        
        loop {
            tokio::select! {
                _ = update_interval.tick() => {
                    self.update_markets().await;
                }
                _ = &mut ctrl_c => {
                    info!("📡 Recibida señal Ctrl+C, deteniendo...");
                    break;
                }
            }
        }
        
        self.stop().await?;
        Ok(())
    }
}