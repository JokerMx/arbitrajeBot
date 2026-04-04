// src/api/mod.rs - Agregar estas líneas en la función api_routes()

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