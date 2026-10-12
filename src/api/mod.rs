//! Módulo de API REST y WebSocket

use warp::Filter;
use std::sync::Arc;
use tokio::sync::Mutex;

pub mod models;
pub mod handlers;
pub mod server;

use crate::bot::TradingBot;
use crate::risk::RiskManager;
use crate::types::{StrategyConfig, RiskConfig, MarketConfig};
use crate::notifier::TelegramNotifier;
use crate::telegram_bot::TelegramBot;
use crate::db::ConfigRepository;
use crate::db::models::ProtectionConfigDB;


pub fn api_routes(
    bot: Arc<Mutex<TradingBot>>,
    risk_manager: Arc<Mutex<RiskManager>>,
    strategy_config: Arc<Mutex<StrategyConfig>>,
    risk_config: Arc<Mutex<RiskConfig>>,
    market_configs: Arc<Mutex<Vec<MarketConfig>>>,
    notifier: Arc<TelegramNotifier>,
    telegram_bot: Option<Arc<TelegramBot>>,
    config_repo: Arc<ConfigRepository>,
    exchange_type: String,
    _protection_config: ProtectionConfigDB,
    leverage_configs: Arc<Mutex<std::collections::HashMap<String, i32>>>,
    protection_arc: Arc<Mutex<ProtectionConfigDB>>,
    telegram_config: Arc<Mutex<crate::db::models::TelegramConfigDB>>,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    
    let exchange_type_arc = Arc::new(Mutex::new(exchange_type));
    
    let state = handlers::ApiState {
        bot,
        risk_manager,
        strategy_config,
        risk_config,
        market_configs,
        notifier,
        telegram_bot,
        config_repo: Some(config_repo),
        bot_start_time: std::time::Instant::now(),
        exchange_type: exchange_type_arc,
        leverage_configs,
        protection_config: protection_arc,
        telegram_config,
    };
    
    let state_filter = warp::any().map(move || state.clone());
    
    // =============================================
    // RUTAS DE ESTADÍSTICAS
    // =============================================
    
    let stats = warp::path!("api" / "stats")
        .and(warp::get())
        .and(state_filter.clone())
        .and_then(handlers::get_stats);
    
    // =============================================
    // RUTAS DE CONFIGURACIÓN
    // =============================================
    
    let config = warp::path!("api" / "config")
        .and(warp::get())
        .and(state_filter.clone())
        .and_then(handlers::get_config);
    
    let update_config = warp::path!("api" / "config")
        .and(warp::post())
        .and(warp::body::json())
        .and(state_filter.clone())
        .and_then(handlers::update_config);
    
    // =============================================
    // RUTAS DE CONTROL DEL BOT
    // =============================================
    
    let start = warp::path!("api" / "bot" / "start")
        .and(warp::post())
        .and(state_filter.clone())
        .and_then(handlers::start_bot);
    
    let stop = warp::path!("api" / "bot" / "stop")
        .and(warp::post())
        .and(state_filter.clone())
        .and_then(handlers::stop_bot);
    
    let restart = warp::path!("api" / "bot" / "restart")
        .and(warp::post())
        .and(state_filter.clone())
        .and_then(handlers::restart_bot);
    
    // =============================================
    // RUTAS DE EXCHANGE
    // =============================================
    
    let update_exchange = warp::path!("api" / "exchange")
        .and(warp::post())
        .and(warp::body::json())
        .and(state_filter.clone())
        .and_then(handlers::update_exchange);
    
    // =============================================
    // RUTAS DE TELEGRAM
    // =============================================
    
    let telegram_users = warp::path!("api" / "telegram" / "users")
        .and(warp::get())
        .and(state_filter.clone())
        .and_then(handlers::get_telegram_users);
    
    let telegram_stats = warp::path!("api" / "telegram" / "stats")
        .and(warp::get())
        .and(state_filter.clone())
        .and_then(handlers::get_telegram_stats);
    
    let telegram_authorize = warp::path!("api" / "telegram" / "users" / String / "authorize")
        .and(warp::post())
        .and(state_filter.clone())
        .and_then(handlers::authorize_telegram_user);
    
    let telegram_deauthorize = warp::path!("api" / "telegram" / "users" / String / "deauthorize")
        .and(warp::post())
        .and(state_filter.clone())
        .and_then(handlers::deauthorize_telegram_user);
    
    let telegram_subscribe = warp::path!("api" / "telegram" / "users" / String / "subscribe")
        .and(warp::post())
        .and(state_filter.clone())
        .and_then(handlers::subscribe_telegram_user);
    
    let telegram_unsubscribe = warp::path!("api" / "telegram" / "users" / String / "unsubscribe")
        .and(warp::post())
        .and(state_filter.clone())
        .and_then(handlers::unsubscribe_telegram_user);
    
    let telegram_delete = warp::path!("api" / "telegram" / "users" / String)
        .and(warp::delete())
        .and(state_filter.clone())
        .and_then(handlers::delete_telegram_user);
    
    let telegram_broadcast = warp::path!("api" / "telegram" / "broadcast")
        .and(warp::post())
        .and(warp::body::json())
        .and(state_filter.clone())
        .and_then(handlers::send_telegram_broadcast);
    
    let telegram_grant = warp::path!("api" / "telegram" / "users" / String / "premium")
        .and(warp::post())
        .and(warp::body::json())
        .and(state_filter.clone())
        .and_then(handlers::grant_telegram_premium);

    let telegram_revoke = warp::path!("api" / "telegram" / "users" / String / "premium")
        .and(warp::delete())
        .and(state_filter.clone())
        .and_then(handlers::revoke_telegram_premium);

    let telegram_payments = warp::path!("api" / "telegram" / "payments")
        .and(warp::get())
        .and(state_filter.clone())
        .and_then(handlers::get_telegram_payments);

    let signal_performance = warp::path!("api" / "signals" / "performance")
        .and(warp::get())
        .and(state_filter.clone())
        .and_then(handlers::get_signal_performance);

    let signal_history = warp::path!("api" / "signals")
        .and(warp::get())
        .and(state_filter.clone())
        .and_then(handlers::get_signal_history);

    let trend_symbols_get = warp::path!("api" / "trend" / "symbols")
        .and(warp::get())
        .and(state_filter.clone())
        .and_then(handlers::get_trend_symbols);

    let trend_symbols_set = warp::path!("api" / "trend" / "symbols")
        .and(warp::put())
        .and(warp::body::json())
        .and(state_filter.clone())
        .and_then(handlers::set_trend_symbols);

    let trend_symbols_reset = warp::path!("api" / "trend" / "symbols")
        .and(warp::delete())
        .and(state_filter.clone())
        .and_then(handlers::reset_trend_symbols);

    let telegram_commands = warp::path!("api" / "telegram" / "commands")
        .and(warp::get())
        .and(state_filter.clone())
        .and_then(handlers::get_telegram_commands);
    
    // =============================================
    // WEBSOCKET
    // =============================================
    
    let ws = warp::path("ws")
        .and(warp::ws())
        .and(state_filter.clone())
        .and_then(server::websocket_handler);
    
    // =============================================
    // ARCHIVOS ESTÁTICOS DEL DASHBOARD
    // =============================================
    
    let static_files = warp::path("dashboard")
        .and(warp::fs::dir("./web_dashboard/dist"));
    
    let dashboard_index = warp::path("dashboard")
        .and(warp::path::end())
        .and(warp::fs::file("./web_dashboard/dist/index.html"));
    
    let dashboard_redirect = warp::path("dashboard")
        .and(warp::path::end())
        .map(|| {
            warp::redirect::redirect(warp::http::Uri::from_static("/dashboard/"))
        });
    
    let root = warp::path::end()
        .and(warp::get())
        .map(|| {
            warp::redirect::redirect(warp::http::Uri::from_static("/dashboard/"))
        });
    
    // =============================================
    // CORS
    // =============================================
    
    let cors = warp::cors()
        .allow_any_origin()
        .allow_methods(vec!["GET", "POST", "PUT", "DELETE", "OPTIONS"])
        .allow_headers(vec!["Content-Type", "Authorization"]);
    
    // =============================================
    // COMBINAR TODAS LAS RUTAS
    // =============================================
    
    // Token de administración (ADMIN_API_TOKEN). Si no está definido, la API queda abierta
    // (solo aceptable escuchando en 127.0.0.1).
    let api_token = std::env::var("ADMIN_API_TOKEN").ok().filter(|t| !t.trim().is_empty());
    if api_token.is_none() {
        tracing::warn!("⚠️ ADMIN_API_TOKEN no definido: la API de administración no pide autenticación");
    }
    let auth = warp::header::optional::<String>("authorization")
        .and_then(move |header: Option<String>| {
            let expected = api_token.clone().map(|t| format!("Bearer {}", t));
            async move {
                match expected {
                    None => Ok(()),
                    Some(e) if header.as_deref() == Some(e.as_str()) => Ok(()),
                    _ => Err(warp::reject::custom(handlers::Unauthorized)),
                }
            }
        })
        .untuple_one();

    let api = auth.and(stats
        .or(config)
        .or(update_config)
        .or(start)
        .or(stop)
        .or(restart)
        .or(update_exchange)
        .or(telegram_users)
        .or(telegram_stats)
        .or(telegram_authorize)
        .or(telegram_deauthorize)
        .or(telegram_subscribe)
        .or(telegram_unsubscribe)
        .or(telegram_delete)
        .or(telegram_broadcast)
        .or(telegram_commands)
        .or(telegram_grant)
        .or(telegram_revoke)
        .or(telegram_payments)
        .or(signal_performance)
        .or(signal_history)
        .or(trend_symbols_get)
        .or(trend_symbols_set)
        .or(trend_symbols_reset));

    let routes = api
        .or(ws)
        .or(dashboard_index)
        .or(static_files)
        .or(dashboard_redirect)
        .or(root)
        .with(cors)
        .recover(handlers::handle_rejection);
    
    routes
}