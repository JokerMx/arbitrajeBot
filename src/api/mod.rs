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

pub fn api_routes(
    bot: Arc<Mutex<TradingBot>>,
    risk_manager: Arc<Mutex<RiskManager>>,
    strategy_config: Arc<Mutex<StrategyConfig>>,
    risk_config: Arc<Mutex<RiskConfig>>,
    market_configs: Arc<Mutex<Vec<MarketConfig>>>,
    notifier: Arc<TelegramNotifier>,
    telegram_bot: Option<Arc<TelegramBot>>,  // Usamos Option<Arc<TelegramBot>>
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    
    let state = handlers::ApiState {
        bot,
        risk_manager,
        strategy_config,
        risk_config,
        market_configs,
        notifier,
        telegram_bot,  // Pasar directamente el Option
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
    // RUTAS DE TELEGRAM - GESTIÓN DE USUARIOS
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
    
    let routes = stats
        .or(config)
        .or(update_config)
        .or(start)
        .or(stop)
        .or(restart)
        .or(telegram_users)
        .or(telegram_stats)
        .or(telegram_authorize)
        .or(telegram_deauthorize)
        .or(telegram_subscribe)
        .or(telegram_unsubscribe)
        .or(telegram_delete)
        .or(telegram_broadcast)
        .or(telegram_commands)
        .or(ws)
        .or(dashboard_index)
        .or(static_files)
        .or(dashboard_redirect)
        .or(root)
        .with(cors)
        .recover(handlers::handle_rejection);
    
    routes
}