//! Hyperliquid Market Making Bot - PRODUCCIÓN MAINNET
//! Con persistencia en MariaDB y límites proporcionales al capital

use dotenv::dotenv;
use std::env;
use tracing_subscriber;
use tracing::{info, warn, error};
use std::sync::Arc;
use tokio::sync::Mutex;
use rust_decimal::prelude::{ToPrimitive, FromPrimitive};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;

mod types;
mod strategy;
mod market;
mod risk;
mod executor;
mod bot;
mod notifier;
mod api;
mod telegram_bot;
mod db;

use crate::bot::{TradingBot, BotConfig};
use crate::types::{MarketConfig, StrategyConfig, RiskConfig};
use crate::api::api_routes;
use crate::telegram_bot::TelegramBot;
use crate::db::{Database, ConfigRepository};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv().ok();
    
    tracing_subscriber::fmt()
        .with_env_filter(
            env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string())
        )
        .init();
    
    info!("================================================");
    info!("  Hyperliquid Market Making Bot v0.1.0");
    info!("  PRODUCCIÓN - MAINNET");
    info!("================================================");
    
    // =============================================
    // INICIALIZAR BASE DE DATOS
    // =============================================
    
    info!("🗄️ Conectando a MariaDB...");
    let db = Database::new().await?;
    db.init_tables().await?;
    
    if !db.health_check().await {
        error!("❌ No se pudo conectar a la base de datos");
    }
    
    let config_repo = ConfigRepository::new(&db);
    
    // =============================================
    // CARGAR CONFIGURACIÓN DESDE BD
    // =============================================
    
    info!("📦 Cargando configuración desde base de datos...");
    let full_config = config_repo.get_full_config().await?;
    
    info!("💰 Capital inicial: ${:.2}", full_config.initial_capital);
    
    // =============================================
    // CARGAR CONFIGURACIÓN DE .ENV
    // =============================================
    
    let private_key = env::var("PRIVATE_KEY")
        .expect("PRIVATE_KEY must be set in .env");
    let wallet = env::var("WALLET")
        .unwrap_or_else(|_| "".to_string());
    let network = env::var("NETWORK")
        .unwrap_or_else(|_| "mainnet".to_string());
    let log_level = env::var("LOG_LEVEL").unwrap_or_else(|_| "info".to_string());
    
    // =============================================
    // CONVERTIR CONFIGURACIÓN DE BD A TIPOS DEL SISTEMA
    // =============================================
    
    let strategy_config = StrategyConfig {
        name: "AvellanedaStoikov".to_string(),
        base_spread_ticks: full_config.strategy.base_spread_ticks as u32,
        risk_aversion: full_config.strategy.risk_aversion.to_f64().unwrap_or(0.15),
        market_impact: full_config.strategy.market_impact.to_f64().unwrap_or(0.01),
        inventory_skew_strength: full_config.strategy.inventory_skew_strength.to_f64().unwrap_or(0.15),
        volatility_lookback_minutes: full_config.strategy.volatility_lookback_minutes as u64,
        order_refresh_time_ms: full_config.strategy.order_refresh_ms as u64,
        max_inventory_ratio: full_config.strategy.max_inventory_ratio.to_f64().unwrap_or(0.10),
    };
    
    let risk_config = RiskConfig {
        max_position_size_pct: full_config.risk.max_position_size_pct.to_f64().unwrap_or(0.10),
        max_daily_loss_pct: full_config.risk.max_daily_loss_pct.to_f64().unwrap_or(0.20),
        max_drawdown_pct: full_config.risk.max_drawdown_pct.to_f64().unwrap_or(0.15),
        kill_switch_enabled: full_config.risk.kill_switch_enabled,
        circuit_breaker_threshold: full_config.risk.circuit_breaker_threshold.to_f64().unwrap_or(0.03),
    };
    
    let initial_capital = full_config.initial_capital;
    
    let mut market_configs = Vec::new();
    for market in full_config.markets {
        if !market.enabled {
            continue;
        }
        market_configs.push(MarketConfig {
            symbol: market.symbol.clone(),
            enabled: market.enabled,
            margin_alloc: market.margin_alloc.to_f64().unwrap_or(0.10),
            min_size: market.min_size.to_f64().unwrap_or(0.00001),
            price_decimals: market.price_decimals as u32,
            size_decimals: market.size_decimals as u32,
        });
    }
    
    // =============================================
    // CONFIGURACIÓN DE TELEGRAM DESDE BD
    // =============================================
    
    let telegram_token = full_config.telegram.bot_token.clone().unwrap_or_default();
    let telegram_chat_id = full_config.telegram.chat_id.clone().unwrap_or_default();
    let telegram_notify_orders = full_config.telegram.notify_orders;
    let telegram_notify_fills = full_config.telegram.notify_fills;
    let telegram_notify_status = full_config.telegram.notify_status;
    
    // =============================================
    // CONFIGURACIÓN DEL EXCHANGE - PRODUCCIÓN MAINNET
    // =============================================
    
    let exchange_type_str = env::var("EXCHANGE_TYPE").unwrap_or_else(|_| "hyperliquid_mainnet".to_string());
    let exchange_type = match exchange_type_str.as_str() {
        "hyperliquid_mainnet" => executor::ExchangeType::HyperliquidMainnet,
        "hyperliquid_testnet" => executor::ExchangeType::HyperliquidTestnet,
        "simulated" => executor::ExchangeType::Simulated,
        _ => executor::ExchangeType::HyperliquidMainnet,
    };
    
    info!("🔄 Modo de exchange: {}", exchange_type_str);
    
    // =============================================
    // CONFIGURACIÓN DEL BOT
    // =============================================
    
    let bot_config = BotConfig::new(
        private_key.clone(),
        wallet,
        network.clone(),
        log_level,
    );
    
    // Calcular límites en USD para mostrar
    let max_position_usd = initial_capital * Decimal::from_f64(risk_config.max_position_size_pct).unwrap_or(dec!(0.10));
    let max_daily_loss_usd = initial_capital * Decimal::from_f64(risk_config.max_daily_loss_pct).unwrap_or(dec!(0.20));
    
    info!("💰 Capital inicial: ${:.2}", initial_capital);
    info!("📊 Límite por posición: ${:.2} ({}% del capital)", max_position_usd, risk_config.max_position_size_pct * 100.0);
    info!("📉 Límite pérdida diaria: ${:.2} ({}% del capital)", max_daily_loss_usd, risk_config.max_daily_loss_pct * 100.0);
    info!("📉 Drawdown máximo: {}%", risk_config.max_drawdown_pct * 100.0);
    info!("🎯 Spread base: {} ticks", strategy_config.base_spread_ticks);
    info!("🌐 Red: {}", network);
    info!("🗄️ Persistencia: MariaDB con prefijo '{}'", db.config.table_prefix);
    
    if let executor::ExchangeType::HyperliquidMainnet = exchange_type {
        info!("⚠️ ⚠️ ⚠️ IMPORTANTE ⚠️ ⚠️ ⚠️");
        info!("   ESTÁS OPERANDO EN MAINNET CON DINERO REAL");
        info!("   Capital: ${:.2} USDC", initial_capital);
        info!("   Riesgo máximo por operación: ${:.2}", max_position_usd);
        info!("⚠️ ⚠️ ⚠️ ⚠️ ⚠️ ⚠️ ⚠️ ⚠️ ⚠️ ⚠️ ⚠️ ⚠️");
    }
    
    // =============================================
    // INICIALIZAR NOTIFICADOR DE TELEGRAM
    // =============================================
    
    let notifier = Arc::new(notifier::TelegramNotifier::new_with_config(
        &telegram_token,
        &telegram_chat_id,
        telegram_notify_orders,
        telegram_notify_fills,
        telegram_notify_status,
    ));
    
    // Enviar notificación de inicio
    if notifier.is_enabled().await {
        let start_msg = format!(
            "🚀 *BOT INICIADO EN MAINNET*\n\n\
             💰 Capital: ${:.2}\n\
             📊 Límite posición: ${:.2}\n\
             🎯 Spread: {} ticks\n\
             ⚠️ Monitorear constantemente",
            initial_capital, max_position_usd, strategy_config.base_spread_ticks
        );
        let _ = notifier.send_message(&start_msg).await;
    }
    
    // =============================================
    // CREAR UNA ÚNICA INSTANCIA DE RISK MANAGER
    // =============================================
    
    info!("📊 Creando RiskManager UNIFICADO...");
    let risk_manager = Arc::new(Mutex::new(risk::RiskManager::new(
        risk_config.clone(),
        initial_capital,
    )));
    
    // =============================================
    // INICIALIZAR BOT CON EL MISMO RISK MANAGER
    // =============================================
    
    let is_testnet = network == "testnet";
    let executor = Arc::new(executor::OrderExecutor::new(
        &bot_config.private_key,
        is_testnet,
        exchange_type,
    ).await?);
    
    info!("🤖 Creando TradingBot...");
    let bot = TradingBot::new(
        bot_config,
        market_configs.clone(),
        strategy_config.clone(),
        risk_config.clone(),
        initial_capital,
        notifier.clone(),
        risk_manager.clone(),
    ).await?;
    
    let bot = Arc::new(Mutex::new(bot));
    
    // =============================================
    // INICIAR TELEGRAM BOT (si hay token)
    // =============================================
    
    let telegram_bot_option: Option<Arc<TelegramBot>> = if !telegram_token.is_empty() {
        info!("🤖 Iniciando Telegram Bot...");
        let telegram_bot = Arc::new(TelegramBot::new(
            &telegram_token,
            risk_manager.clone(),
            bot.clone(),
            notifier.clone(),
        ));
        
        if !telegram_chat_id.is_empty() {
            telegram_bot.add_admin(&telegram_chat_id).await;
            info!("👑 Administrador añadido: {}", telegram_chat_id);
        }
        
        let telegram_bot_clone = telegram_bot.clone();
        tokio::spawn(async move {
            telegram_bot_clone.run().await;
        });
        
        info!("✅ Telegram Bot iniciado correctamente");
        Some(telegram_bot)
    } else {
        warn!("⚠️ TELEGRAM_BOT_TOKEN no configurado");
        None
    };
    
    // =============================================
    // INICIAR SERVIDOR API
    // =============================================
    
    let strategy_config_arc = Arc::new(Mutex::new(strategy_config));
    let risk_config_arc = Arc::new(Mutex::new(risk_config));
    let market_configs_arc = Arc::new(Mutex::new(market_configs));
    let config_repo_arc = Some(Arc::new(config_repo));
    let bot_start_time = std::time::Instant::now();
    
    info!("🌐 Iniciando servidor API REST y WebSocket...");
    
    let api_routes = api_routes(
        bot.clone(),
        risk_manager.clone(),
        strategy_config_arc,
        risk_config_arc,
        market_configs_arc,
        notifier.clone(),
        telegram_bot_option,
        config_repo_arc,
        bot_start_time,
    );
    
    let port = 3001;
    info!("🚀 Servidor API iniciado en http://localhost:{}", port);
    info!("📊 Dashboard disponible en http://localhost:{}/dashboard", port);
    info!("📡 WebSocket disponible en ws://localhost:{}/ws", port);
    info!("📱 API Telegram en http://localhost:{}/api/telegram", port);
    info!("🗄️ Base de datos: MariaDB en {}", db.config.url);
    
    if let executor::ExchangeType::HyperliquidMainnet = exchange_type {
        info!("💹 OPERANDO EN MAINNET CON DINERO REAL");
        info!("   Monitorear en: http://localhost:{}/dashboard", port);
    }
    
    let bot_clone_for_run = bot.clone();
    
    tokio::select! {
        result = async {
            let bot_guard = bot_clone_for_run.lock().await;
            bot_guard.clone().run_until_signal().await
        } => {
            if let Err(e) = result {
                error!("❌ Error en el bot: {}", e);
                
                // Notificar error por Telegram
                if notifier.is_enabled().await {
                    let _ = notifier.send_message(&format!("🚨 *ERROR CRÍTICO*\n\n{}", e)).await;
                }
            }
        }
        _ = warp::serve(api_routes).run(([127, 0, 0, 1], port)) => {
            info!("🛑 Servidor API detenido");
        }
    }
    
    info!("👋 Bot finalizado");
    
    // Notificar parada
    if notifier.is_enabled().await {
        let _ = notifier.send_message("🛑 Bot detenido correctamente").await;
    }
    
    Ok(())
}