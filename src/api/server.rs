//! Servidor WebSocket para comunicación en tiempo real

use warp::ws::{Message, WebSocket};
use futures::{StreamExt, SinkExt};
use tracing::{info, warn, error, debug};
use rust_decimal::prelude::ToPrimitive;

use super::models::*;
use super::handlers::ApiState;

pub async fn websocket_handler(
    ws: warp::ws::Ws,
    state: ApiState,
) -> Result<impl warp::Reply, warp::Rejection> {
    info!("📡 Nueva conexión WebSocket solicitada");
    Ok(ws.on_upgrade(move |socket| handle_websocket(socket, state)))
}

async fn handle_websocket(ws: WebSocket, state: ApiState) {
    info!("📡 Cliente WebSocket conectado");
    
    let (mut ws_tx, mut ws_rx) = ws.split();
    
    let telegram_tx = state.notifier.init_broadcast().await;
    let mut telegram_rx = telegram_tx.subscribe();
    
    let (update_tx, mut update_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
    
    // Heartbeats cada 30 segundos
    let heartbeat_tx = update_tx.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(30));
        loop {
            interval.tick().await;
            if heartbeat_tx.send(r#"{"type":"heartbeat"}"#.to_string()).is_err() {
                break;
            }
            debug!("💓 Heartbeat enviado");
        }
    });
    
    // Stats cada 3 segundos
    let stats_tx = update_tx.clone();
    let state_stats = state.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(3));
        
        loop {
            interval.tick().await;
            
            // Obtener datos del RiskManager
            let risk = state_stats.risk_manager.lock().await;
            
            let balance = risk.current_balance().to_f64().unwrap_or(0.0);
            let pnl = risk.total_pnl().to_f64().unwrap_or(0.0);
            let initial = risk.initial_capital().to_f64().unwrap_or(10.0);
            
            // Obtener trades recientes
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
            
            // Obtener posiciones reales del RiskManager
            let positions: Vec<PositionResponse> = risk.get_positions()  // Usar el nuevo getter
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
            
            info!("📊 Posiciones abiertas: {}", positions.len());
            
            // CALCULAR WIN RATE
            let winning_trades = risk.winning_trades();
            let losing_trades = risk.losing_trades();
            let total_trades = winning_trades + losing_trades;
            let win_rate = if total_trades > 0 {
                (winning_trades as f64 / total_trades as f64) * 100.0
            } else {
                0.0
            };
            
            info!("📊 WIN RATE CALCULADO: {:.1}% ({}G/{}P)", win_rate, winning_trades, losing_trades);
            
            // Calcular drawdown
            let peak = risk.peak_balance().to_f64().unwrap_or(balance);
            let drawdown = if peak > 0.0 {
                ((peak - balance) / peak) * 100.0
            } else {
                0.0
            };
            
            let volume = risk.total_volume().to_f64().unwrap_or(0.0);
            
            drop(risk);
            
            // Crear historial
            let history = vec![
                HistoryPoint {
                    timestamp: chrono::Utc::now().to_rfc3339(),
                    balance,
                    pnl,
                }
            ];
            
            // Crear stats con TODOS los campos
            let stats = StatsResponse {
                balance,
                pnl,
                pnl_percent: if initial > 0.0 { (pnl / initial) * 100.0 } else { 0.0 },
                daily_pnl: 0.0,
                drawdown,
                win_rate,
                total_trades: total_trades as u32,
                winning_trades,
                losing_trades,
                volume,
                positions,  // <-- AHORA ENVIAMOS POSICIONES REALES
                trades: trades.clone(),
                history,
            };
            
            info!("📊 ENVIANDO STATS - Balance: ${:.2}, WinRate: {:.1}%, Posiciones: {}", 
                  stats.balance, stats.win_rate, stats.positions.len());
            
            match serde_json::to_string(&WebSocketMessage::Stats(stats)) {
                Ok(json) => {
                    if stats_tx.send(json).is_err() {
                        error!("❌ Canal de stats cerrado");
                        break;
                    }
                }
                Err(e) => {
                    error!("❌ Error serializando stats: {}", e);
                }
            }
        }
    });
    
    // Reenviar SOLO fills al dashboard
    let forward_tx = update_tx.clone();
    tokio::spawn(async move {
        while let Ok(msg) = telegram_rx.recv().await {
            if msg.starts_with("📊 FILL:") {
                if let Ok(json) = serde_json::to_string(&WebSocketMessage::Alert(msg)) {
                    let _ = forward_tx.send(json);
                }
            }
        }
    });
    
    // Enviar actualizaciones al cliente
    let send_task = tokio::spawn(async move {
        while let Some(message) = update_rx.recv().await {
            if ws_tx.send(Message::text(message)).await.is_err() {
                break;
            }
        }
    });
    
    // Recibir mensajes del cliente
    let receive_task = tokio::spawn(async move {
        while let Some(result) = ws_rx.next().await {
            if let Err(e) = result {
                warn!("⚠️ Error en WebSocket: {}", e);
                break;
            }
        }
    });
    
    tokio::select! {
        _ = receive_task => {}
        _ = send_task => {}
    }
}