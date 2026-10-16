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
    pub signal_history: Arc<Mutex<crate::signals::SignalHistory>>,
    pub equity_series: Arc<Mutex<crate::signals::EquitySeries>>,
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
            // Nunca exponer el token completo
            bot_token: telegram
                .bot_token
                .as_deref()
                .filter(|t| t.len() > 10)
                .map(|t| format!("{}…{}", &t[..6], &t[t.len() - 4..]))
                .unwrap_or_default(),
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
    
    // Actualizar estrategia (market making: se aplica al reiniciar)
    if let Some(strat_update) = update.strategy {
        let mut strategy = state.strategy_config.lock().await;
        if let Some(v) = strat_update.base_spread_ticks { strategy.base_spread_ticks = v; }
        if let Some(v) = strat_update.risk_aversion { strategy.risk_aversion = v; }
        if let Some(v) = strat_update.market_impact { strategy.market_impact = v; }
        if let Some(v) = strat_update.inventory_skew_strength { strategy.inventory_skew_strength = v; }
        if let Some(v) = strat_update.volatility_lookback_minutes { strategy.volatility_lookback_minutes = v; }
        if let Some(v) = strat_update.order_refresh_ms { strategy.order_refresh_time_ms = v; }
        if let Some(v) = strat_update.max_inventory_ratio { strategy.max_inventory_ratio = v; }
        if let Some(repo) = &state.config_repo {
            let db = crate::db::models::StrategyConfigDB {
                base_spread_ticks: strategy.base_spread_ticks as i32,
                risk_aversion: Decimal::from_f64(strategy.risk_aversion).unwrap_or_default(),
                market_impact: Decimal::from_f64(strategy.market_impact).unwrap_or_default(),
                inventory_skew_strength: Decimal::from_f64(strategy.inventory_skew_strength).unwrap_or_default(),
                volatility_lookback_minutes: strategy.volatility_lookback_minutes as i64,
                order_refresh_ms: strategy.order_refresh_time_ms as i64,
                max_inventory_ratio: Decimal::from_f64(strategy.max_inventory_ratio).unwrap_or_default(),
            };
            if let Err(e) = repo.update_strategy_config(&db).await {
                tracing::warn!("No se pudo guardar la estrategia en la BD: {}", e);
            }
        }
    }
    
    // Actualizar riesgo (usando porcentajes)
    if let Some(risk_update) = update.risk {
        let mut risk = state.risk_config.lock().await;
        if let Some(v) = risk_update.max_position_size_pct { risk.max_position_size_pct = v; }
        if let Some(v) = risk_update.max_daily_loss_pct { risk.max_daily_loss_pct = v; }
        if let Some(v) = risk_update.max_drawdown_pct { risk.max_drawdown_pct = v; }
        if let Some(v) = risk_update.kill_switch_enabled { risk.kill_switch_enabled = v; }
        if let Some(v) = risk_update.circuit_breaker_threshold { risk.circuit_breaker_threshold = v; }
        // Aplicar en vivo al risk manager y guardar en la BD
        state.risk_manager.lock().await.set_config(risk.clone());
        if let Some(repo) = &state.config_repo {
            let db = crate::db::models::RiskConfigDB {
                max_position_size_pct: Decimal::from_f64(risk.max_position_size_pct).unwrap_or_default(),
                max_daily_loss_pct: Decimal::from_f64(risk.max_daily_loss_pct).unwrap_or_default(),
                max_drawdown_pct: Decimal::from_f64(risk.max_drawdown_pct).unwrap_or_default(),
                kill_switch_enabled: risk.kill_switch_enabled,
                circuit_breaker_threshold: Decimal::from_f64(risk.circuit_breaker_threshold).unwrap_or_default(),
            };
            if let Err(e) = repo.update_risk_config(&db).await {
                tracing::warn!("No se pudo guardar el riesgo en la BD: {}", e);
            }
        }
    }

    // Actualizar Telegram (se aplica al reiniciar el bot)
    if let Some(tg) = update.telegram {
        let mut cfg = state.telegram_config.lock().await;
        if let Some(v) = tg.enabled { cfg.enabled = v; }
        // El dashboard recibe el token enmascarado: solo se guarda si es uno nuevo
        if let Some(v) = tg.bot_token.filter(|t| !t.trim().is_empty() && !t.contains('…') && !t.contains('•')) {
            cfg.bot_token = Some(v.trim().to_string());
        }
        if let Some(v) = tg.chat_id.filter(|c| !c.trim().is_empty()) { cfg.chat_id = Some(v.trim().to_string()); }
        if let Some(v) = tg.notify_orders { cfg.notify_orders = v; }
        if let Some(v) = tg.notify_fills { cfg.notify_fills = v; }
        if let Some(v) = tg.notify_status { cfg.notify_status = v; }
        if let Some(repo) = &state.config_repo {
            if let Err(e) = repo.update_telegram_config(&cfg).await {
                tracing::warn!("No se pudo guardar Telegram en la BD: {}", e);
            }
        }
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

/// Estado del bot para el encabezado del dashboard
pub async fn bot_status(state: ApiState) -> Result<impl Reply, Rejection> {
    let (mode, live, paused) = {
        let bot = state.bot.lock().await;
        (
            bot.strategy_mode(),
            bot.trend_is_live(),
            bot.paused_handle().map(|p| p.load(std::sync::atomic::Ordering::Relaxed)),
        )
    };
    let rebound_live = live && std::env::var("REBOUND_LIVE").map_or(false, |v| v.trim() == "true");
    let risk = {
        let rm = state.risk_manager.lock().await;
        serde_json::json!({
            "status": format!("{:?}", rm.status()),
            "kill_switch": rm.is_kill_switch_engaged(),
            "drawdown_pct": (rm.current_drawdown() * Decimal::from(100)).to_f64(),
        })
    };
    Ok(json(&serde_json::json!({
        "strategy_mode": mode,
        "live": live,
        "rebound_live": rebound_live,
        "paused": paused.unwrap_or(false),
        "can_pause": paused.is_some(),
        "uptime_secs": state.bot_start_time.elapsed().as_secs(),
        "version": env!("CARGO_PKG_VERSION"),
        "risk": risk,
    })))
}

async fn set_paused(state: &ApiState, value: bool) -> Result<warp::reply::Json, Rejection> {
    let handle = state.bot.lock().await.paused_handle();
    let Some(handle) = handle else {
        return Ok(json(&serde_json::json!({
            "success": false,
            "message": "La pausa solo está disponible con la estrategia de tendencia"
        })));
    };
    handle.store(value, std::sync::atomic::Ordering::Relaxed);
    if let Some(repo) = &state.config_repo {
        let _ = repo.set_config_value("trading_paused", if value { "true" } else { "false" }, "bool").await;
    }
    let msg = if value {
        "Bot en pausa: no abrirá nuevas posiciones (las abiertas se siguen gestionando)"
    } else {
        "Bot reanudado: vuelve a abrir posiciones"
    };
    info!("{} {}", if value { "⏸️" } else { "▶️" }, msg);
    let _ = state.notifier.send_message(&format!("{} {}", if value { "⏸️" } else { "▶️" }, msg)).await;
    Ok(json(&serde_json::json!({ "success": true, "paused": value, "message": msg })))
}

/// Reanuda nuevas entradas
pub async fn start_bot(state: ApiState) -> Result<impl Reply, Rejection> {
    set_paused(&state, false).await
}

/// Pausa nuevas entradas (no cierra posiciones)
pub async fn stop_bot(state: ApiState) -> Result<impl Reply, Rejection> {
    set_paused(&state, true).await
}

/// Reinicia el proceso: Docker/systemd lo vuelven a levantar (restart policy)
pub async fn restart_bot(state: ApiState) -> Result<impl Reply, Rejection> {
    let _ = state.notifier.send_message("🔄 Reiniciando el bot desde el dashboard...").await;
    tokio::spawn(async {
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        tracing::warn!("🔄 Reinicio solicitado desde el dashboard");
        std::process::exit(0);
    });
    Ok(json(&serde_json::json!({
        "status": "success",
        "success": true,
        "message": "Reiniciando: el bot vuelve en ~30 segundos"
    })))
}

// =============================================
// CONFIGURACIÓN DE ESTRATEGIAS (tendencia + rebote)
// =============================================

fn strategy_json(t: &crate::trend::TrendConfig, r: &crate::trend::ReboundConfig) -> serde_json::Value {
    let f = |d: Decimal| d.to_f64().unwrap_or(0.0);
    serde_json::json!({
        "trend_risk_per_trade": f(t.risk_per_trade),
        "trend_max_risk_per_trade": f(t.max_risk_per_trade),
        "trend_max_positions": t.max_positions,
        "trend_max_leverage": f(t.max_total_leverage),
        "trend_breakout_hours": t.breakout_hours,
        "trend_exit_hours": t.exit_hours,
        "trend_stop_atr": f(t.stop_atr),
        "trend_funding_filter": t.funding_filter,
        "rebound_enabled": r.enabled,
        "rebound_live": r.live,
        "rebound_entry_rsi": f(r.entry_rsi),
        "rebound_exit_rsi": f(r.exit_rsi),
        "rebound_max_hold_hours": r.max_hold_hours,
        "rebound_risk_per_trade": f(r.risk_per_trade),
        "rebound_max_positions": r.max_positions,
    })
}

pub async fn get_strategy_config(state: ApiState) -> Result<impl Reply, Rejection> {
    let handle = state.bot.lock().await.trend_config_handle();
    let Some(handle) = handle else {
        return Ok(json(&serde_json::json!({ "enabled": false })));
    };
    let (t, r) = handle.read().await.clone();
    let (et, er) = (crate::trend::TrendConfig::from_env(), crate::trend::ReboundConfig::from_env());
    let saved = match &state.config_repo {
        Some(repo) => repo.get_config_value("strategy_overrides").await.ok().flatten(),
        None => None,
    };
    Ok(json(&serde_json::json!({
        "enabled": true,
        "active": strategy_json(&t, &r),
        "env_default": strategy_json(&et, &er),
        "source": if saved.is_some() { "dashboard" } else { "env" },
    })))
}

pub async fn set_strategy_config(body: serde_json::Value, state: ApiState) -> Result<impl Reply, Rejection> {
    let overrides: crate::trend::StrategyOverrides = match serde_json::from_value(body) {
        Ok(o) => o,
        Err(e) => {
            return Ok(json(&serde_json::json!({ "success": false, "message": format!("Datos inválidos: {}", e) })))
        }
    };
    let handle = state.bot.lock().await.trend_config_handle();
    let Some(handle) = handle else {
        return Ok(json(&serde_json::json!({ "success": false, "message": "La estrategia de tendencia no está activa" })));
    };
    let (t, r) = overrides.apply(&crate::trend::TrendConfig::from_env(), &crate::trend::ReboundConfig::from_env());
    if let Some(repo) = &state.config_repo {
        let raw = serde_json::to_string(&overrides).unwrap_or_default();
        if let Err(e) = repo.set_config_value("strategy_overrides", &raw, "json").await {
            return Ok(json(&serde_json::json!({ "success": false, "message": format!("Error guardando: {}", e) })));
        }
    }
    *handle.write().await = (t.clone(), r.clone());
    info!("⚙️ Configuración de estrategias actualizada desde el dashboard");
    Ok(json(&serde_json::json!({
        "success": true,
        "message": "Guardado. Se aplica en la próxima evaluación horaria.",
        "active": strategy_json(&t, &r),
    })))
}

pub async fn reset_strategy_config(state: ApiState) -> Result<impl Reply, Rejection> {
    if let Some(repo) = &state.config_repo {
        let _ = repo.delete_config_value("strategy_overrides").await;
    }
    let (t, r) = (crate::trend::TrendConfig::from_env(), crate::trend::ReboundConfig::from_env());
    if let Some(handle) = state.bot.lock().await.trend_config_handle() {
        *handle.write().await = (t.clone(), r.clone());
    }
    Ok(json(&serde_json::json!({
        "success": true,
        "message": "Restablecido a los valores del .env",
        "active": strategy_json(&t, &r),
    })))
}

// =============================================
// SISTEMA
// =============================================

pub async fn system_info(state: ApiState) -> Result<impl Reply, Rejection> {
    let env = |k: &str| std::env::var(k).unwrap_or_default();
    let mask = |s: String| {
        if s.len() > 10 { format!("{}…{}", &s[..6], &s[s.len() - 4..]) } else if s.is_empty() { "—".into() } else { "••••".into() }
    };
    let (mode, live) = {
        let bot = state.bot.lock().await;
        (bot.strategy_mode(), bot.trend_is_live())
    };
    let initial = state.risk_manager.lock().await.initial_capital().to_f64();
    let exchange = state.exchange_type.lock().await.clone();
    Ok(json(&serde_json::json!({
        "exchange_type": exchange,
        "live": live,
        "rebound_live": live && env("REBOUND_LIVE").trim() == "true",
        "strategy_mode": mode,
        "network": env("NETWORK"),
        "wallet": mask(env("WALLET")),
        "initial_capital": initial,
        "data_dir": crate::signals::data_dir().display().to_string(),
        "log_level": env("RUST_LOG"),
        "api_token_set": !env("ADMIN_API_TOKEN").trim().is_empty(),
        "uptime_secs": state.bot_start_time.elapsed().as_secs(),
        "version": env!("CARGO_PKG_VERSION"),
    })))
}

/// Cambia el capital inicial (simulado). Se aplica al reiniciar.
pub async fn set_initial_capital(body: serde_json::Value, state: ApiState) -> Result<impl Reply, Rejection> {
    let Some(capital) = body["capital"].as_f64().filter(|c| *c >= 10.0 && *c <= 10_000_000.0) else {
        return Ok(json(&serde_json::json!({ "success": false, "message": "Capital inválido (mínimo $10)" })));
    };
    let Some(repo) = &state.config_repo else {
        return Ok(json(&serde_json::json!({ "success": false, "message": "Base de datos no disponible" })));
    };
    match repo.update_initial_capital(Decimal::from_f64(capital).unwrap_or_default()).await {
        Ok(_) => Ok(json(&serde_json::json!({
            "success": true,
            "message": format!("Capital inicial ${:.2} guardado. Reinicia el bot para aplicarlo.", capital),
            "restart_required": true,
        }))),
        Err(e) => Ok(json(&serde_json::json!({ "success": false, "message": format!("Error: {}", e) }))),
    }
}

/// Mensaje de prueba solo al chat del administrador
pub async fn telegram_test(state: ApiState) -> Result<impl Reply, Rejection> {
    match state
        .notifier
        .send_message("✅ Mensaje de prueba desde el dashboard: el bot puede escribirte por Telegram.")
        .await
    {
        Ok(_) if state.notifier.is_enabled().await => Ok(json(&serde_json::json!({
            "success": true,
            "message": "Mensaje de prueba enviado a tu chat de administrador"
        }))),
        Ok(_) => Ok(json(&serde_json::json!({
            "success": false,
            "message": "Telegram no está configurado (falta token o chat id)"
        }))),
        Err(e) => Ok(json(&serde_json::json!({ "success": false, "message": e }))),
    }
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
        Some(bot) => {
            let all = bot.get_performance().await;
            let (trend, rebound) = bot.get_performance_by_strategy().await;
            // Mismos campos de siempre + desglose por estrategia
            let mut v = serde_json::to_value(&all).unwrap_or_default();
            v["by_strategy"] = serde_json::json!({ "tendencia": trend, "rebote": rebound });
            Ok(json(&v))
        }
        None => Ok(json(&serde_json::to_value(crate::signals::Performance::default()).unwrap_or_default())),
    }
}

pub async fn get_signal_history(state: ApiState) -> Result<impl Reply, Rejection> {
    match &state.telegram_bot {
        Some(bot) => Ok(json(&bot.get_signals(200).await)),
        None => Ok(json(&Vec::<crate::signals::SignalRecord>::new())),
    }
}

// =============================================
// DIARIO DE OPERACIONES (página principal)
// =============================================

pub async fn get_journal(state: ApiState) -> Result<impl Reply, Rejection> {
    let (initial, equity_now) = {
        let rm = state.risk_manager.lock().await;
        (
            rm.initial_capital().to_f64().unwrap_or(0.0),
            rm.current_balance().to_f64().unwrap_or(0.0),
        )
    };
    let journal = state.signal_history.lock().await.journal(initial);
    let mut v = serde_json::to_value(&journal).unwrap_or_default();

    // Precio actual y P&L no realizado de las posiciones abiertas
    let client = state.bot.lock().await.market_client();
    if let (Some(client), Some(open)) = (client, v["open"].as_array_mut()) {
        if !open.is_empty() {
            if let Ok(mids) = client.get_all_mids().await {
                for o in open.iter_mut() {
                    let symbol = o["symbol"].as_str().unwrap_or_default().to_string();
                    let entry = o["entry"].as_f64().unwrap_or(0.0);
                    if let Some(px) = mids.get(&symbol).and_then(|d| d.to_f64()) {
                        let pct = if entry > 0.0 { (px / entry - 1.0) * 100.0 } else { 0.0 };
                        o["current"] = serde_json::json!(px);
                        o["unrealized_pct"] = serde_json::json!(pct);
                        if let Some(n) = o["notional"].as_f64() {
                            o["unrealized_usd"] = serde_json::json!(n * pct / 100.0);
                        }
                    }
                }
            }
        }
    }
    // Fotos de equity (5 min los últimos 7 días, luego 1/h) de los últimos 30 días
    v["equity_series"] = serde_json::to_value(state.equity_series.lock().await.last_hours(24 * 30))
        .unwrap_or_default();
    v["initial_capital"] = serde_json::json!(initial);
    v["equity_now"] = serde_json::json!(equity_now);
    v["updated_at"] = serde_json::json!(chrono::Utc::now());
    Ok(json(&v))
}

// =============================================
// MONEDAS DE LA ESTRATEGIA DE TENDENCIA
// =============================================

const MAX_TREND_SYMBOLS: usize = 30;

pub async fn get_trend_symbols(state: ApiState) -> Result<impl Reply, Rejection> {
    let (handle, available) = {
        let bot = state.bot.lock().await;
        (bot.trend_symbols_handle(), bot.trend_available_markets())
    };
    let active = match &handle {
        Some(h) => h.read().await.clone(),
        None => Vec::new(),
    };
    let saved = match &state.config_repo {
        Some(repo) => repo.get_trend_symbols().await.ok().flatten(),
        None => None,
    };
    Ok(json(&serde_json::json!({
        "enabled": handle.is_some(),
        "active": active,
        "source": if saved.is_some() { "dashboard" } else { "env" },
        "env_default": crate::trend::env_symbols(),
        "available": available,
        "max": MAX_TREND_SYMBOLS,
    })))
}

pub async fn set_trend_symbols(body: serde_json::Value, state: ApiState) -> Result<impl Reply, Rejection> {
    let fail = |m: String| -> Result<warp::reply::Json, Rejection> {
        Ok(json(&serde_json::json!({ "success": false, "message": m })))
    };
    let (handle, available) = {
        let bot = state.bot.lock().await;
        (bot.trend_symbols_handle(), bot.trend_available_markets())
    };
    let Some(handle) = handle else {
        return fail("La estrategia de tendencia no está activa (STRATEGY_MODE)".to_string());
    };
    let mut symbols: Vec<String> = Vec::new();
    for v in body["symbols"].as_array().cloned().unwrap_or_default() {
        if let Some(s) = v.as_str() {
            let s = s.trim().to_uppercase();
            if !s.is_empty() && !symbols.contains(&s) {
                symbols.push(s);
            }
        }
    }
    if symbols.is_empty() {
        return fail("Selecciona al menos una moneda".to_string());
    }
    if symbols.len() > MAX_TREND_SYMBOLS {
        return fail(format!("Máximo {} monedas", MAX_TREND_SYMBOLS));
    }
    let unknown: Vec<&String> = symbols.iter().filter(|s| !available.contains(s)).collect();
    if !unknown.is_empty() {
        return fail(format!("No existen en Hyperliquid: {:?}", unknown));
    }
    if let Some(repo) = &state.config_repo {
        if let Err(e) = repo.set_trend_symbols(&symbols).await {
            return fail(format!("Error guardando en la BD: {}", e));
        }
    }
    *handle.write().await = symbols.clone();
    info!("📈 Monedas de tendencia actualizadas desde el dashboard: {}", symbols.join(","));
    Ok(json(&serde_json::json!({
        "success": true,
        "message": format!("{} monedas activas; se aplican en la próxima evaluación horaria", symbols.len()),
        "active": symbols,
    })))
}

pub async fn reset_trend_symbols(state: ApiState) -> Result<impl Reply, Rejection> {
    let (handle, available) = {
        let bot = state.bot.lock().await;
        (bot.trend_symbols_handle(), bot.trend_available_markets())
    };
    if let Some(repo) = &state.config_repo {
        let _ = repo.clear_trend_symbols().await;
    }
    let defaults: Vec<String> = crate::trend::env_symbols()
        .into_iter()
        .filter(|s| available.contains(s))
        .collect();
    if let Some(h) = handle {
        if !defaults.is_empty() {
            *h.write().await = defaults.clone();
        }
    }
    Ok(json(&serde_json::json!({
        "success": true,
        "message": "Restablecido a TREND_SYMBOLS del .env",
        "active": defaults,
    })))
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