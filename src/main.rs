// warp: la cadena de rutas .or() supera el límite de recursión por defecto
#![recursion_limit = "256"]
//! Hyperliquid Market Making Bot - PRODUCCIÓN MAINNET (API REST + firma)

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
mod trend;
mod signals;
mod market;
mod risk;
mod executor;
mod bot;
mod notifier;
mod api;
mod telegram_bot;
mod db;
mod hyperliquid;
mod bybit;

use crate::bot::{TradingBot, BotConfig};
use crate::types::{MarketConfig, StrategyConfig, RiskConfig};
use crate::api::api_routes;
use crate::telegram_bot::TelegramBot;
use crate::db::{Database, ConfigRepository};
use crate::executor::ExchangeType;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string()))
        .init();

    info!("================================================");
    info!("  Hyperliquid Market Making Bot v0.1.0");
    info!("  PRODUCCIÓN - MAINNET (API REST + FIRMA)");
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
    let private_key = env::var("PRIVATE_KEY").expect("PRIVATE_KEY must be set");
    let wallet = env::var("WALLET").unwrap_or_default();
    let network = env::var("NETWORK").unwrap_or_else(|_| "mainnet".to_string());
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

    // Construir lista de mercados (solo los habilitados)
    let mut market_configs = Vec::new();
    for market in full_config.markets {
        if !market.enabled {
            continue;
        }
        let margin = market.margin_alloc.to_f64().unwrap_or(0.10);
        let min_s = market.min_size.to_f64().unwrap_or(0.00001);
        market_configs.push(MarketConfig {
            symbol: market.symbol.clone(),
            asset_index: 0,
            enabled: market.enabled,
            margin_alloc: if margin.is_finite() && margin > 0.0 { margin } else { 0.10 },
            min_size: if min_s.is_finite() && min_s > 0.0 { min_s } else { 0.00001 },
            price_decimals: market.price_decimals as u32,
            size_decimals: market.size_decimals as u32,
        });
    }

    // =============================================
    // CONFIGURACIÓN DE TELEGRAM
    // =============================================
    let telegram_token = full_config.telegram.bot_token.clone().unwrap_or_default();
    let telegram_chat_id = full_config.telegram.chat_id.clone().unwrap_or_default();
    let telegram_notify_orders = full_config.telegram.notify_orders;
    let telegram_notify_fills = full_config.telegram.notify_fills;
    let telegram_notify_status = full_config.telegram.notify_status;

    // =============================================
    // TIPO DE EXCHANGE (MAINNET, BYBIT o SIMULADO)
    // =============================================
    let exchange_type_str = env::var("EXCHANGE_TYPE").unwrap_or_else(|_| "simulated".to_string());
    let exchange_type = match exchange_type_str.as_str() {
        "hyperliquid_mainnet" => ExchangeType::HyperliquidMainnet,
        "bybit" => ExchangeType::Bybit,
        _ => ExchangeType::Simulated,
    };
    info!("🔄 Modo de exchange: {}", exchange_type_str);

    // =============================================
    // CREDENCIALES DE BYBIT
    // =============================================
    let bybit_api_key = env::var("BYBIT_API_KEY").ok();
    let bybit_api_secret = env::var("BYBIT_API_SECRET").ok();
    let bybit_testnet = env::var("BYBIT_TESTNET")
        .unwrap_or_else(|_| "false".to_string())
        .parse::<bool>()
        .unwrap_or(false);
    
    if bybit_testnet {
        info!("🧪 BYBIT TESTNET ACTIVADO");
    }

    // =============================================
    // CONFIGURACIÓN DEL BOT
    // =============================================
    let bot_config = BotConfig::new(private_key.clone(), wallet, network.clone(), log_level);

    let max_position_usd = initial_capital
        * Decimal::from_f64(risk_config.max_position_size_pct).unwrap_or(dec!(0.10));
    let max_daily_loss_usd = initial_capital
        * Decimal::from_f64(risk_config.max_daily_loss_pct).unwrap_or(dec!(0.20));

    info!("💰 Capital inicial: ${:.2}", initial_capital);
    info!(
        "📊 Límite por posición: ${:.2} ({}% del capital)",
        max_position_usd,
        risk_config.max_position_size_pct * 100.0
    );
    info!(
        "📉 Límite pérdida diaria: ${:.2} ({}% del capital)",
        max_daily_loss_usd,
        risk_config.max_daily_loss_pct * 100.0
    );
    info!("📉 Drawdown máximo: {}%", risk_config.max_drawdown_pct * 100.0);
    info!("🎯 Spread base: {} ticks", strategy_config.base_spread_ticks);
    info!("🌐 Red: {}", network);
    info!("🗄️ Persistencia: MariaDB con prefijo '{}'", db.config.table_prefix);
    let exchange_type_str = env::var("EXCHANGE_TYPE").unwrap_or_else(|_| "simulated".to_string());
    println!("🔧 EXCHANGE_TYPE leído: '{}'", exchange_type_str);
    info!("🔄 Modo de exchange: {}", exchange_type_str);
    if let ExchangeType::HyperliquidMainnet = exchange_type {
        info!("⚠️ ⚠️ ⚠️ IMPORTANTE ⚠️ ⚠️ ⚠️");
        info!("   ESTÁS OPERANDO EN MAINNET CON DINERO REAL");
        info!("   Capital: ${:.2} USDC", initial_capital);
        info!("   Riesgo máximo por operación: ${:.2}", max_position_usd);
        info!("⚠️ ⚠️ ⚠️ ⚠️ ⚠️ ⚠️ ⚠️ ⚠️ ⚠️ ⚠️ ⚠️ ⚠️");
    }

    // =============================================
    // NOTIFICADOR TELEGRAM
    // =============================================
    let notifier = Arc::new(notifier::TelegramNotifier::new_with_config(
        &telegram_token,
        &telegram_chat_id,
        telegram_notify_orders,
        telegram_notify_fills,
        telegram_notify_status,
    ));

    if notifier.is_enabled().await {
        let start_msg = format!(
            "🚀 *BOT INICIADO EN MAINNET*\n\n💰 Capital: ${:.2}\n📊 Límite posición: ${:.2}\n🎯 Spread: {} ticks\n⚠️ Monitorear constantemente",
            initial_capital, max_position_usd, strategy_config.base_spread_ticks
        );
        let _ = notifier.send_message(&start_msg).await;
    }

    // =============================================
    // RISK MANAGER
    // =============================================
    info!("📊 Creando RiskManager UNIFICADO...");
    let mut risk_manager_instance = risk::RiskManager::new(
        risk_config.clone(),
        initial_capital,
    );
    
    risk_manager_instance.set_protection(full_config.protection.clone());
    
    for lev in &full_config.leverage {
        risk_manager_instance.set_leverage(&lev.exchange, &lev.symbol, lev.leverage);
    }
    
    let risk_manager = Arc::new(Mutex::new(risk_manager_instance));

    // =============================================
    // EJECUTOR (con API REST + firma)
    // =============================================
    let executor = Arc::new(executor::OrderExecutor::new(&private_key, exchange_type, bybit_api_key.as_deref(), bybit_api_secret.as_deref(), bybit_testnet).await?);

    // Si es mainnet, obtener asset_index de los mercados reales
    if matches!(exchange_type, executor::ExchangeType::HyperliquidMainnet) {
        for market in &mut market_configs {
            if let Some(asset_idx) = executor.get_asset_index(&market.symbol) {
                market.asset_index = asset_idx;
                info!("✅ {} tiene asset_index: {}", market.symbol, asset_idx);
            } else {
                warn!("⚠️ No se encontró mercado real para {}", market.symbol);
            }
        }
    }

    // =============================================
    // TRADING BOT
    // =============================================
    info!("🤖 Creando TradingBot...");
    let bot = TradingBot::new(
        bot_config,
        market_configs.clone(),
        strategy_config.clone(),
        risk_config.clone(),
        initial_capital,
        notifier.clone(),
        risk_manager.clone(),
        exchange_type,
    )
    .await?;
    let bot = Arc::new(Mutex::new(bot));

    // =============================================
    // TELEGRAM BOT
    // =============================================
    let telegram_bot_option: Option<Arc<TelegramBot>> = if !telegram_token.is_empty() {
        info!("🤖 Iniciando Telegram Bot...");
        let tg_bot = Arc::new(TelegramBot::new(
            &telegram_token,
            risk_manager.clone(),
            bot.clone(),
            notifier.clone(),
        ));
        if !telegram_chat_id.is_empty() {
            tg_bot.add_admin(&telegram_chat_id).await;
            info!("👑 Administrador añadido: {}", telegram_chat_id);
        }
        let tg_bot_clone = tg_bot.clone();
        tokio::spawn(async move {
            tg_bot_clone.run().await;
        });
        info!("✅ Telegram Bot iniciado correctamente");
        Some(tg_bot)
    } else {
        warn!("⚠️ TELEGRAM_BOT_TOKEN no configurado");
        None
    };

    // =============================================
    // API SERVER
    // =============================================
    let strategy_config_arc = Arc::new(Mutex::new(strategy_config));
    let risk_config_arc = Arc::new(Mutex::new(risk_config));
    let market_configs_arc = Arc::new(Mutex::new(market_configs));

    info!("🌐 Iniciando servidor API REST y WebSocket...");
    let config_repo_arc = Arc::new(config_repo);
    
    let exchange_type_arc = Arc::new(Mutex::new(exchange_type_str.clone()));
    let leverage_configs = Arc::new(Mutex::new({
        let mut map = std::collections::HashMap::new();
        for lev in &full_config.leverage {
            let key = format!("{}_{}", lev.exchange, lev.symbol);
            map.insert(key, lev.leverage);
        }
        map
    }));
    let protection_arc = Arc::new(Mutex::new(full_config.protection.clone()));
    let telegram_arc = Arc::new(Mutex::new(full_config.telegram.clone()));
    
    let api_routes = api_routes(
        bot.clone(),
        risk_manager.clone(),
        strategy_config_arc,
        risk_config_arc,
        market_configs_arc,
        notifier.clone(),
        telegram_bot_option,
        config_repo_arc,
        exchange_type_str.clone(),
        full_config.protection.clone(),
        leverage_configs,
        protection_arc,
        telegram_arc,
    );

    // API_BIND=0.0.0.0 solo dentro de un contenedor; en un servidor deja 127.0.0.1
    let port: u16 = env::var("API_PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(3001);
    let bind: std::net::IpAddr = env::var("API_BIND")
        .ok()
        .and_then(|b| b.parse().ok())
        .unwrap_or(std::net::IpAddr::from([127, 0, 0, 1]));
    if !bind.is_loopback() && env::var("ADMIN_API_TOKEN").map_or(true, |t| t.trim().is_empty()) {
        warn!("⚠️ API expuesta en {} SIN ADMIN_API_TOKEN: cualquiera en la red puede administrarla", bind);
    }
    info!("🚀 Servidor API iniciado en http://localhost:{}", port);
    info!("📊 Dashboard disponible en http://localhost:{}/dashboard", port);
    info!("📡 WebSocket disponible en ws://localhost:{}/ws", port);
    info!("📱 API Telegram en http://localhost:{}/api/telegram", port);
    info!("🗄️ Base de datos: MariaDB en {}", db.config.redacted_url());

    if let ExchangeType::HyperliquidMainnet = exchange_type {
        info!("💹 OPERANDO EN MAINNET CON DINERO REAL");
        info!("   Monitorear en: http://localhost:{}/dashboard", port);
    }

    let bot_clone_for_run = bot.clone();

    tokio::select! {
        result = async {
            // Clonar y soltar el lock: si se retiene, la API (start/stop/restart) se bloquea para siempre
            let bot_to_run = bot_clone_for_run.lock().await.clone();
            bot_to_run.run_until_signal().await
        } => {
            if let Err(e) = result {
                error!("❌ Error en el bot: {}", e);
                if notifier.is_enabled().await {
                    let _ = notifier.send_message(&format!("🚨 *ERROR CRÍTICO*\n\n{}", e)).await;
                }
            }
        }
        _ = warp::serve(api_routes).run((bind, port)) => {
            info!("🛑 Servidor API detenido");
        }
    }

    info!("👋 Bot finalizado");
    if notifier.is_enabled().await {
        let _ = notifier.send_message("🛑 Bot detenido correctamente").await;
    }

    Ok(())
}