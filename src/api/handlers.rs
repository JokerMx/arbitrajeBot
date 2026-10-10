//! Manejadores para las rutas de la API

use warp::{Reply, Rejection, reply::json};
use std::sync::Arc;
use tokio::sync::Mutex;
use rust_decimal::prelude::ToPrimitive;

use crate::bot::TradingBot;
use crate::risk::RiskManager;
use crate::types::{StrategyConfig, RiskConfig, MarketConfig};
use crate::notifier::TelegramNotifier;
use crate::telegram_bot::TelegramBot;
use crate::db::ConfigRepository;
use tracing::info;
use rust_decimal::Decimal;
use rust_decimal::prelude::FromPrimitive;
use super::models::*;

// =============================================
// ESTADO COMPARTIDO
// =============================================

#[derive(Clone)]
pub struct ApiState {
    pub bot: Arc<Mutex<TradingBot>>,
    pub risk_manager: Arc<Mutex<RiskManager>>,
    pub strategy_config: Arc<Mutex<StrategyConfig>>,
    pub risk_config: Arc<Mutex<RiskConfig>>,
    pub market_configs: Arc<Mutex<Vec<MarketConfig>>>,
    pub notifier: Arc<TelegramNotifier>,
    pub telegram_bot: Option<Arc<TelegramBot>>,
    pub config_repo: Option<Arc<ConfigRepository>>,
    pub bot_start_time: std::time::Instant,
    pub exchange_type: Arc<Mutex<String>>,
    pub leverage_configs: Arc<Mutex<std::collections::HashMap<String, i32>>>,
    pub protection_config: Arc<Mutex<crate::db::models::ProtectionConfigDB>>,
    pub telegram_config: Arc<Mutex<crate::db::models::TelegramConfigDB>>,
}

// =============================================
// HANDLERS DE ESTADÍSTICAS
// =============================================

pub async fn get_stats(state: ApiState) -> Result<impl Reply, Rejection> {
    let risk = state.risk_manager.lock().await;
    
    let trades: Vec<TradeResponse> = risk.recent_trades(50)
        .iter()
        .map(|t| TradeResponse {
            id: t.order_id.clone(),
            symbol: t.symbol.clone(),
            side: format!("{}", t.side),
            size: t.size.to_f64().unwrap_or(0.0),
            price: t.price.to_f64().unwrap_or(0.0),
            value: (t.size * t.price).to_f64().unwrap_or(0.0),
            pnl: t.pnl.to_f64().unwrap_or(0.0),
            timestamp: t.timestamp.to_rfc3339(),
        })
        .collect();
    
    let positions: Vec<PositionResponse> = risk.get_positions()
        .iter()
        .map(|p| PositionResponse {
            symbol: p.symbol.clone(),
            size: p.size.to_f64().unwrap_or(0.0),
            entry_price: p.entry_price.to_f64().unwrap_or(0.0),
            current_price: p.current_price.to_f64().unwrap_or(0.0),
            unrealized_pnl: p.unrealized_pnl.to_f64().unwrap_or(0.0),
            realized_pnl: p.realized_pnl.to_f64().unwrap_or(0.0),
            value: (p.size * p.current_price).to_f64().unwrap_or(0.0),
        })
        .collect();
    
    let history = vec![
        HistoryPoint {
            timestamp: chrono::Utc::now().to_rfc3339(),
            balance: risk.current_balance().to_f64().unwrap_or(0.0),
            pnl: risk.total_pnl().to_f64().unwrap_or(0.0),
        }
    ];
    
    let balance = risk.current_balance().to_f64().unwrap_or(0.0);
    let pnl = risk.total_pnl().to_f64().unwrap_or(0.0);
    let initial = risk.initial_capital().to_f64().unwrap_or(10.0);
    let winning_trades = risk.winning_trades();
    let losing_trades = risk.losing_trades();
    let total_trades = winning_trades + losing_trades;
    let win_rate = if total_trades > 0 {
        (winning_trades as f64 / total_trades as f64) * 100.0
    } else {
        0.0
    };
    
    let response = StatsResponse {
        balance,
        pnl,
        pnl_percent: if initial > 0.0 { (pnl / initial) * 100.0 } else { 0.0 },
        daily_pnl: risk.daily_pnl().to_f64().unwrap_or(0.0),
        drawdown: risk.current_drawdown().to_f64().unwrap_or(0.0) * 100.0,
        win_rate,
        total_trades: total_trades as u32,
        winning_trades,
        losing_trades,
        volume: risk.total_volume().to_f64().unwrap_or(0.0),
        positions,
        trades,
        history,
    };
    
    Ok(json(&response))
}

// =============================================
// HANDLERS DE CONFIGURACIÓN
// =============================================

pub async fn get_config(state: ApiState) -> Result<impl Reply, Rejection> {
    let strategy = state.strategy_config.lock().await;
    let risk = state.risk_config.lock().await;
    let markets = state.market_configs.lock().await;
    let risk_mgr = state.risk_manager.lock().await;
    let leverage_configs = state.leverage_configs.lock().await;
    let protection = state.protection_config.lock().await;
    let telegram = state.telegram_config.lock().await;
    
    info!("📊 get_config: {} mercados, leverage configs: {}", markets.len(), leverage_configs.len());
    
    let leverage: Vec<LeverageResponse> = leverage_configs.iter()
        .map(|(k, v)| {
            let parts: Vec<&str> = k.split('_').collect();
            let exchange = parts.get(0).unwrap_or(&"simulated");
            let symbol = parts.get(1).unwrap_or(&"BTC");
            LeverageResponse {
                exchange: exchange.to_string(),
                symbol: symbol.to_string(),
                leverage: *v,
                max_leverage: protection.max_leverage,
            }
        })
        .collect();
    
    let response = FullConfigResponse {
        strategy: StrategyConfigResponse {
            base_spread_ticks: strategy.base_spread_ticks,
            risk_aversion: strategy.risk_aversion,
            market_impact: strategy.market_impact,
            inventory_skew_strength: strategy.inventory_skew_strength,
            volatility_lookback_minutes: strategy.volatility_lookback_minutes,
            order_refresh_ms: strategy.order_refresh_time_ms,
            max_inventory_ratio: strategy.max_inventory_ratio,
        },
        risk: RiskConfigResponse {
            max_position_size_pct: risk.max_position_size_pct,
            max_daily_loss_pct: risk.max_daily_loss_pct,
            max_drawdown_pct: risk.max_drawdown_pct,
            kill_switch_enabled: risk.kill_switch_enabled,
            circuit_breaker_threshold: risk.circuit_breaker_threshold,
        },
        markets: markets.iter().map(|m| MarketConfigResponse {
            symbol: m.symbol.clone(),
            enabled: m.enabled,
            margin_alloc: m.margin_alloc,
            min_size: m.min_size,
        }).collect(),
        telegram: TelegramConfigResponse {
            enabled: telegram.enabled,
            bot_token: telegram.bot_token.clone().unwrap_or_default(),
            chat_id: telegram.chat_id.clone().unwrap_or_default(),
            notify_orders: telegram.notify_orders,
            notify_fills: telegram.notify_fills,
            notify_status: telegram.notify_status,
        },
        initial_capital: risk_mgr.initial_capital().to_f64().unwrap_or(10.0),
        leverage,
        protection: ProtectionResponse {
            max_capital_at_risk_pct: protection.max_capital_at_risk_pct.to_f64().unwrap_or(0.25),
            auto_stop_loss_enabled: protection.auto_stop_loss_enabled,
            auto_stop_loss_pct: protection.auto_stop_loss_pct.to_f64().unwrap_or(0.15),
            max_leverage: protection.max_leverage,
            emergency_withdraw_enabled: protection.emergency_withdraw_enabled,
        },
    };
    
    Ok(json(&response))
}

pub async fn update_config(
    update: FullConfigUpdate,
    state: ApiState,
) -> Result<impl Reply, Rejection> {
    
    // Actualizar estrategia
    if let Some(strat_update) = update.strategy {
        let mut strategy = state.strategy_config.lock().await;
        if let Some(v) = strat_update.base_spread_ticks { strategy.base_spread_ticks = v; }
        if let Some(v) = strat_update.risk_aversion { strategy.risk_aversion = v; }
        if let Some(v) = strat_update.market_impact { strategy.market_impact = v; }
        if let Some(v) = strat_update.inventory_skew_strength { strategy.inventory_skew_strength = v; }
        if let Some(v) = strat_update.volatility_lookback_minutes { strategy.volatility_lookback_minutes = v; }
        if let Some(v) = strat_update.order_refresh_ms { strategy.order_refresh_time_ms = v; }
        if let Some(v) = strat_update.max_inventory_ratio { strategy.max_inventory_ratio = v; }
    }
    
    // Actualizar riesgo (usando porcentajes)
    if let Some(risk_update) = update.risk {
        let mut risk = state.risk_config.lock().await;
        if let Some(v) = risk_update.max_position_size_pct { risk.max_position_size_pct = v; }
        if let Some(v) = risk_update.max_daily_loss_pct { risk.max_daily_loss_pct = v; }
        if let Some(v) = risk_update.max_drawdown_pct { risk.max_drawdown_pct = v; }
        if let Some(v) = risk_update.kill_switch_enabled { risk.kill_switch_enabled = v; }
        if let Some(v) = risk_update.circuit_breaker_threshold { risk.circuit_breaker_threshold = v; }
    }
    
    // Actualizar mercados
    if let Some(market_updates) = update.markets {
        let mut markets = state.market_configs.lock().await;
        for update in market_updates {
            if let Some(market) = markets.iter_mut().find(|m| m.symbol == update.symbol) {
                if let Some(v) = update.enabled { market.enabled = v; }
                if let Some(v) = update.margin_alloc { market.margin_alloc = v; }
                if let Some(v) = update.min_size { market.min_size = v; }
            }
        }
        
        // Guardar en base de datos
        if let Some(repo) = &state.config_repo {
            let markets_list: Vec<MarketConfig> = markets.clone();
            let repo_clone = repo.clone();
            tokio::spawn(async move {
                for market in &markets_list {
                    let _ = repo_clone.update_market(
                        &market.symbol,
                        Some(market.enabled),
                        Some(Decimal::from_f64(market.margin_alloc).unwrap_or_default()),
                        Some(Decimal::from_f64(market.min_size).unwrap_or_default()),
                    ).await;
                }
                info!("✅ Mercados actualizados en BD");
            });
        }
    }
    
    // Actualizar apalancamiento
    if let Some(leverage_updates) = update.leverage {
        let mut lev_configs = state.leverage_configs.lock().await;
        let mut risk_mgr = state.risk_manager.lock().await;
        for lev in &leverage_updates {
            let key = format!("{}_{}", lev.exchange, lev.symbol);
            lev_configs.insert(key.clone(), lev.leverage);
            risk_mgr.set_leverage(&lev.exchange, &lev.symbol, lev.leverage);
        }
        
        if let Some(repo) = &state.config_repo {
            let repo_clone = repo.clone();
            let levs = leverage_updates.clone();
            tokio::spawn(async move {
                for lev in levs {
                    let _ = repo_clone.update_leverage(&lev.exchange, &lev.symbol, lev.leverage, 10).await;
                }
            });
        }
    }
    
    // Actualizar protección
    if let Some(prot_update) = update.protection {
        let prot_clone = {
            let mut prot = state.protection_config.lock().await;
            if let Some(v) = prot_update.max_capital_at_risk_pct {
                prot.max_capital_at_risk_pct = Decimal::from_f64(v).unwrap_or(Decimal::new(25, 2));
            }
            if let Some(v) = prot_update.auto_stop_loss_enabled {
                prot.auto_stop_loss_enabled = v;
            }
            if let Some(v) = prot_update.auto_stop_loss_pct {
                prot.auto_stop_loss_pct = Decimal::from_f64(v).unwrap_or(Decimal::new(15, 2));
            }
            if let Some(v) = prot_update.max_leverage {
                prot.max_leverage = v;
            }
            if let Some(v) = prot_update.emergency_withdraw_enabled {
                prot.emergency_withdraw_enabled = v;
            }
            
            let mut risk_mgr = state.risk_manager.lock().await;
            risk_mgr.set_protection(prot.clone());
            
            prot.clone()
        };
        
        if let Some(repo) = &state.config_repo {
            let repo_clone = repo.clone();
            let prot_to_save = prot_clone;
            tokio::spawn(async move {
                let _ = repo_clone.update_protection_config(&prot_to_save).await;
            });
        }
    }
    
    Ok(json(&serde_json::json!({
        "status": "success",
        "message": "Configuración actualizada"
    })))
}

// =============================================
// HANDLERS DE CONTROL DEL BOT
// =============================================

pub async fn start_bot(state: ApiState) -> Result<impl Reply, Rejection> {
    let _bot = state.bot.lock().await;
    Ok(json(&serde_json::json!({
        "status": "success",
        "message": "Bot iniciado"
    })))
}

pub async fn stop_bot(state: ApiState) -> Result<impl Reply, Rejection> {
    let _bot = state.bot.lock().await;
    Ok(json(&serde_json::json!({
        "status": "success",
        "message": "Bot detenido"
    })))
}

pub async fn restart_bot(state: ApiState) -> Result<impl Reply, Rejection> {
    let _bot = state.bot.lock().await;
    Ok(json(&serde_json::json!({
        "status": "success",
        "message": "Bot reiniciado"
    })))
}

#[derive(serde::Deserialize, Debug)]
pub struct UpdateExchangeRequest {
    pub exchange_type: Option<String>,
    pub network: Option<String>,
    pub log_level: Option<String>,
}

pub async fn update_exchange(
    update: UpdateExchangeRequest,
    state: ApiState,
) -> Result<impl Reply, Rejection> {
    info!("📡 update_exchange recibido: {:?}", update);
    
    if let Some(exchange_type) = &update.exchange_type {
        info!("🔄 Cambiando exchange_type a: {}", exchange_type);
        let mut current = state.exchange_type.lock().await;
        if *current != *exchange_type {
            info!("🔄 Cambiando exchange: {} -> {}", current, exchange_type);
            *current = exchange_type.clone();
        }
    }
    
    let response = json(&serde_json::json!({
        "status": "success",
        "message": "Exchange actualizado. El cambio se aplicará en ~10 segundos."
    }));
    info!("📤 Respondiendo con success");
    Ok(response)
}

// =============================================
// HANDLERS DE TELEGRAM
// =============================================

pub async fn get_telegram_users(state: ApiState) -> Result<impl Reply, Rejection> {
    if let Some(bot) = &state.telegram_bot {
        let users = bot.get_users().await;
        Ok(json(&users))
    } else {
        Ok(json(&Vec::<crate::telegram_bot::TelegramUser>::new()))
    }
}

pub async fn get_telegram_stats(state: ApiState) -> Result<impl Reply, Rejection> {
    if let Some(bot) = &state.telegram_bot {
        let stats = bot.get_stats().await;
        Ok(json(&stats))
    } else {
        Ok(json(&crate::telegram_bot::TelegramStats::default()))
    }
}

pub async fn authorize_telegram_user(
    chat_id: String,
    state: ApiState,
) -> Result<impl Reply, Rejection> {
    if let Some(bot) = &state.telegram_bot {
        match bot.authorize_user(&chat_id).await {
            Ok(_) => Ok(json(&serde_json::json!({
                "success": true,
                "message": format!("Usuario {} autorizado correctamente", chat_id)
            }))),
            Err(e) => Ok(json(&serde_json::json!({
                "success": false,
                "message": e
            }))),
        }
    } else {
        Ok(json(&serde_json::json!({
            "success": false,
            "message": "Bot de Telegram no disponible"
        })))
    }
}

pub async fn deauthorize_telegram_user(
    chat_id: String,
    state: ApiState,
) -> Result<impl Reply, Rejection> {
    if let Some(bot) = &state.telegram_bot {
        match bot.deauthorize_user(&chat_id).await {
            Ok(_) => Ok(json(&serde_json::json!({
                "success": true,
                "message": format!("Usuario {} desautorizado", chat_id)
            }))),
            Err(e) => Ok(json(&serde_json::json!({
                "success": false,
                "message": e
            }))),
        }
    } else {
        Ok(json(&serde_json::json!({
            "success": false,
            "message": "Bot de Telegram no disponible"
        })))
    }
}

pub async fn subscribe_telegram_user(
    chat_id: String,
    state: ApiState,
) -> Result<impl Reply, Rejection> {
    if let Some(bot) = &state.telegram_bot {
        match bot.subscribe_user(&chat_id).await {
            Ok(_) => Ok(json(&serde_json::json!({
                "success": true,
                "message": format!("Usuario {} suscrito", chat_id)
            }))),
            Err(e) => Ok(json(&serde_json::json!({
                "success": false,
                "message": e
            }))),
        }
    } else {
        Ok(json(&serde_json::json!({
            "success": false,
            "message": "Bot de Telegram no disponible"
        })))
    }
}

pub async fn unsubscribe_telegram_user(
    chat_id: String,
    state: ApiState,
) -> Result<impl Reply, Rejection> {
    if let Some(bot) = &state.telegram_bot {
        match bot.unsubscribe_user(&chat_id).await {
            Ok(_) => Ok(json(&serde_json::json!({
                "success": true,
                "message": format!("Usuario {} desuscrito", chat_id)
            }))),
            Err(e) => Ok(json(&serde_json::json!({
                "success": false,
                "message": e
            }))),
        }
    } else {
        Ok(json(&serde_json::json!({
            "success": false,
            "message": "Bot de Telegram no disponible"
        })))
    }
}

pub async fn delete_telegram_user(
    chat_id: String,
    state: ApiState,
) -> Result<impl Reply, Rejection> {
    if let Some(bot) = &state.telegram_bot {
        match bot.delete_user(&chat_id).await {
            Ok(_) => Ok(json(&serde_json::json!({
                "success": true,
                "message": format!("Usuario {} eliminado", chat_id)
            }))),
            Err(e) => Ok(json(&serde_json::json!({
                "success": false,
                "message": e
            }))),
        }
    } else {
        Ok(json(&serde_json::json!({
            "success": false,
            "message": "Bot de Telegram no disponible"
        })))
    }
}

pub async fn send_telegram_broadcast(
    body: serde_json::Value,
    state: ApiState,
) -> Result<impl Reply, Rejection> {
    if let Some(bot) = &state.telegram_bot {
        let message = body["message"].as_str().unwrap_or("");
        let segment = body["segment"].as_str().unwrap_or("all");
        if message.trim().is_empty() {
            return Ok(json(&serde_json::json!({ "success": false, "message": "Mensaje vacío", "sent": 0 })));
        }
        match bot.broadcast_segment(message, segment).await {
            Ok(sent) => Ok(json(&serde_json::json!({
                "success": true,
                "message": "Broadcast enviado",
                "sent": sent
            }))),
            Err(e) => Ok(json(&serde_json::json!({
                "success": false,
                "message": e,
                "sent": 0
            }))),
        }
    } else {
        Ok(json(&serde_json::json!({
            "success": false,
            "message": "Bot de Telegram no disponible",
            "sent": 0
        })))
    }
}

pub async fn get_telegram_commands(_state: ApiState) -> Result<impl Reply, Rejection> {
    let list: &[(&str, &str, &str)] = &[
        ("/start", "Bienvenida (acepta enlaces de referido)", "public"),
        ("/premium", "Planes y pago con Telegram Stars", "public"),
        ("/trial", "Prueba Premium gratis (una vez)", "public"),
        ("/plan", "Plan actual del usuario", "public"),
        ("/performance", "Resultados de las señales", "public"),
        ("/referral", "Enlace de referido", "public"),
        ("/signals", "Señales abiertas", "premium"),
        ("/subscribe", "Activar avisos", "public"),
        ("/unsubscribe", "Pausar avisos", "public"),
        ("/terms", "Términos del servicio", "public"),
        ("/paysupport", "Soporte de pagos", "public"),
        ("/status", "Estado de la cuenta del bot", "admin"),
        ("/balance", "Balance de la cuenta del bot", "admin"),
        ("/trades", "Últimas señales", "admin"),
        ("/stats", "Usuarios e ingresos", "admin"),
        ("/users", "Listar usuarios", "admin"),
        ("/authorize", "Autorizar usuario", "admin"),
        ("/deauthorize", "Desautorizar usuario", "admin"),
        ("/grant", "Regalar días Premium", "admin"),
        ("/revoke", "Quitar Premium", "admin"),
        ("/refund", "Reembolsar pago en Stars", "admin"),
        ("/broadcast", "Mensaje a todos (también _free / _premium)", "admin"),
    ];
    let commands: Vec<serde_json::Value> = list
        .iter()
        .map(|(c, d, a)| serde_json::json!({ "command": c, "description": d, "access": a }))
        .collect();
    Ok(json(&commands))
}

// =============================================
// MONETIZACIÓN
// =============================================

fn no_telegram() -> warp::reply::Json {
    json(&serde_json::json!({ "success": false, "message": "Bot de Telegram no disponible" }))
}

pub async fn grant_telegram_premium(
    chat_id: String,
    body: serde_json::Value,
    state: ApiState,
) -> Result<impl Reply, Rejection> {
    let Some(bot) = &state.telegram_bot else { return Ok(no_telegram()) };
    let days = body["days"].as_i64().unwrap_or(30);
    Ok(match bot.grant_premium(&chat_id, days).await {
        Ok(until) => json(&serde_json::json!({
            "success": true,
            "message": format!("Premium hasta {}", until.format("%d-%m-%Y")),
            "premium_until": until
        })),
        Err(e) => json(&serde_json::json!({ "success": false, "message": e })),
    })
}

pub async fn revoke_telegram_premium(chat_id: String, state: ApiState) -> Result<impl Reply, Rejection> {
    let Some(bot) = &state.telegram_bot else { return Ok(no_telegram()) };
    Ok(match bot.revoke_premium(&chat_id).await {
        Ok(_) => json(&serde_json::json!({ "success": true, "message": "Premium revocado" })),
        Err(e) => json(&serde_json::json!({ "success": false, "message": e })),
    })
}

pub async fn get_telegram_payments(state: ApiState) -> Result<impl Reply, Rejection> {
    match &state.telegram_bot {
        Some(bot) => Ok(json(&bot.get_payments().await)),
        None => Ok(json(&Vec::<crate::telegram_bot::PaymentRecord>::new())),
    }
}

pub async fn get_signal_performance(state: ApiState) -> Result<impl Reply, Rejection> {
    match &state.telegram_bot {
        Some(bot) => Ok(json(&bot.get_performance().await)),
        None => Ok(json(&crate::signals::Performance::default())),
    }
}

pub async fn get_signal_history(state: ApiState) -> Result<impl Reply, Rejection> {
    match &state.telegram_bot {
        Some(bot) => Ok(json(&bot.get_signals(200).await)),
        None => Ok(json(&Vec::<crate::signals::SignalRecord>::new())),
    }
}

/// Rechazo por falta de token de administración
#[derive(Debug)]
pub struct Unauthorized;
impl warp::reject::Reject for Unauthorized {}

// =============================================
// MANEJADOR DE ERRORES
// =============================================

pub async fn handle_rejection(err: Rejection) -> Result<impl Reply, Rejection> {
    let code;
    let message;
    
    if err.find::<Unauthorized>().is_some() {
        code = warp::http::StatusCode::UNAUTHORIZED;
        message = "No autorizado: falta o es inválido el token (ADMIN_API_TOKEN)";
    } else if err.is_not_found() {
        code = warp::http::StatusCode::NOT_FOUND;
        message = "Ruta no encontrada";
    } else if err.find::<warp::reject::MethodNotAllowed>().is_some() {
        code = warp::http::StatusCode::METHOD_NOT_ALLOWED;
        message = "Método no permitido";
    } else {
        code = warp::http::StatusCode::INTERNAL_SERVER_ERROR;
        message = "Error interno del servidor";
    }
    
    Ok(warp::reply::with_status(
        warp::reply::json(&serde_json::json!({
            "error": message,
            "status": code.as_u16()
        })),
        code,
    ))
}