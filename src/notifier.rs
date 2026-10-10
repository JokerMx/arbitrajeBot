//! Notificador a Telegram - Configurable desde .env

use reqwest::Client;
use serde_json::json;
use tracing::{info, error, warn, debug};
use std::sync::Arc;
use tokio::sync::{Mutex, broadcast};

#[derive(Clone)]
pub struct TelegramNotifier {
    bot_token: String,
    client: Client,
    default_chat_id: String,
    enabled: Arc<Mutex<bool>>,
    notify_orders: Arc<Mutex<bool>>,      // Configurable desde .env
    notify_fills: Arc<Mutex<bool>>,       // Configurable desde .env
    notify_status: Arc<Mutex<bool>>,      // Configurable desde .env
    message_tx: Arc<Mutex<Option<broadcast::Sender<String>>>>,
    recent_fill_ids: Arc<Mutex<Vec<String>>>,
    /// Bus de señales de trading hacia los suscriptores (lo consume el bot de Telegram)
    signal_tx: broadcast::Sender<crate::signals::SignalEvent>,
}

impl TelegramNotifier {
    /// Constructor simple (usa valores por defecto)
    pub fn new(bot_token: &str, chat_id: &str) -> Self {
        let enabled = !bot_token.is_empty() && !chat_id.is_empty();
        
        if enabled {
            info!("📱 Telegram configurado - Token: {}... Chat ID: {}", 
                  &bot_token[..bot_token.len().min(8)], chat_id);
        } else {
            warn!("📱 Telegram NO configurado - Faltan token o chat_id");
        }
        
        Self {
            bot_token: bot_token.to_string(),
            client: Client::new(),
            default_chat_id: chat_id.to_string(),
            enabled: Arc::new(Mutex::new(enabled)),
            notify_orders: Arc::new(Mutex::new(true)),   // Por defecto true
            notify_fills: Arc::new(Mutex::new(true)),    // Por defecto true
            notify_status: Arc::new(Mutex::new(true)),   // Por defecto true
            message_tx: Arc::new(Mutex::new(None)),
            recent_fill_ids: Arc::new(Mutex::new(Vec::with_capacity(100))),
            signal_tx: broadcast::channel(256).0,
        }
    }
    
    /// Constructor con configuración personalizada DESDE .ENV
    pub fn new_with_config(
        bot_token: &str, 
        chat_id: &str, 
        notify_orders: bool,   // Viene del .env
        notify_fills: bool,    // Viene del .env
        notify_status: bool    // Viene del .env
    ) -> Self {
        let enabled = !bot_token.is_empty() && !chat_id.is_empty();
        
        if enabled {
            info!("📱 Telegram configurado - Token: {}... Chat ID: {}", 
                  &bot_token[..bot_token.len().min(8)], chat_id);
            info!("   📨 Configuración de notificaciones:");
            info!("      - Órdenes: {}", if notify_orders { "✅ ACTIVADO" } else { "❌ DESACTIVADO" });
            info!("      - Fills:    {}", if notify_fills { "✅ ACTIVADO" } else { "❌ DESACTIVADO" });
            info!("      - Estado:   {}", if notify_status { "✅ ACTIVADO" } else { "❌ DESACTIVADO" });
        } else {
            warn!("📱 Telegram NO configurado - Faltan token o chat_id");
        }
        
        Self {
            bot_token: bot_token.to_string(),
            client: Client::new(),
            default_chat_id: chat_id.to_string(),
            enabled: Arc::new(Mutex::new(enabled)),
            notify_orders: Arc::new(Mutex::new(notify_orders)),
            notify_fills: Arc::new(Mutex::new(notify_fills)),
            notify_status: Arc::new(Mutex::new(notify_status)),
            message_tx: Arc::new(Mutex::new(None)),
            recent_fill_ids: Arc::new(Mutex::new(Vec::with_capacity(100))),
            signal_tx: broadcast::channel(256).0,
        }
    }
    
    /// Publica una señal para los suscriptores. Si nadie escucha (sin bot de Telegram) se descarta.
    pub fn publish_signal(&self, event: crate::signals::SignalEvent) {
        let _ = self.signal_tx.send(event);
    }

    pub fn subscribe_signals(&self) -> broadcast::Receiver<crate::signals::SignalEvent> {
        self.signal_tx.subscribe()
    }

    /// Inicializar canal de broadcast para logs
    pub async fn init_broadcast(&self) -> broadcast::Sender<String> {
        let (tx, _) = broadcast::channel(100);
        let mut message_tx = self.message_tx.lock().await;
        *message_tx = Some(tx.clone());
        tx
    }
    
    /// Verificar si un fill ya fue enviado
    async fn is_fill_duplicate(&self, fill_id: &str) -> bool {
        let mut recent = self.recent_fill_ids.lock().await;
        if recent.contains(&fill_id.to_string()) {
            debug!("🔄 Fill duplicado detectado y bloqueado: {}", fill_id);
            true
        } else {
            recent.push(fill_id.to_string());
            if recent.len() > 100 {
                recent.remove(0);
            }
            false
        }
    }
    
    /// Enviar mensaje al chat por defecto
    pub async fn send_message(&self, message: &str) -> Result<(), String> {
        self.send_message_to_chat(&self.default_chat_id, message).await
    }
    
    /// Enviar mensaje a un chat específico
    pub async fn send_message_to_chat(&self, chat_id: &str, message: &str) -> Result<(), String> {
        let enabled = self.enabled.lock().await;
        if !*enabled {
            debug!("📱 Telegram desactivado, mensaje no enviado");
            return Ok(());
        }
        drop(enabled);
        
        if self.bot_token.is_empty() {
            error!("❌ Token de Telegram vacío");
            return Err("Token vacío".to_string());
        }
        
        if chat_id.is_empty() {
            error!("❌ Chat ID vacío");
            return Err("Chat ID vacío".to_string());
        }
        
        let url = format!("https://api.telegram.org/bot{}/sendMessage", self.bot_token);
        debug!("📤 Enviando a Telegram: {}", message.replace('\n', " "));
        
        let payload = json!({
            "chat_id": chat_id,
            "text": message,
            "parse_mode": "HTML",
        });
        
        let mut retries = 0;
        let max_retries = 3;
        
        while retries < max_retries {
            match self.client.post(&url).json(&payload).send().await {
                Ok(response) => {
                    let status = response.status();
                    if status.is_success() {
                        debug!("✅ Mensaje enviado a Telegram");
                        return Ok(());
                    } else if status == 429 {
                        retries += 1;
                        let wait_time = 1000 * 2u64.pow(retries);
                        warn!("⚠️ Rate limit (429). Esperando {} ms (intento {}/{})", 
                              wait_time, retries, max_retries);
                        tokio::time::sleep(tokio::time::Duration::from_millis(wait_time)).await;
                    } else {
                        let text = response.text().await.unwrap_or_default();
                        error!("❌ Error Telegram ({}): {}", status, text);
                        return Err(format!("Error {}: {}", status, text));
                    }
                }
                Err(e) => {
                    error!("❌ Error de conexión a Telegram: {}", e);
                    retries += 1;
                    let wait_time = 1000 * 2u64.pow(retries);
                    tokio::time::sleep(tokio::time::Duration::from_millis(wait_time)).await;
                }
            }
        }
        
        Err("Max retries alcanzado".to_string())
    }
    
    /// Enviar fill al dashboard
    pub async fn send_fill_to_dashboard(&self, symbol: &str, side: &str, quantity: f64, price: f64, pnl: f64, fill_id: &str) {
        if self.is_fill_duplicate(fill_id).await {
            return;
        }
        
        if let Some(tx) = self.message_tx.lock().await.as_ref() {
            let timestamp = chrono::Local::now().format("%H:%M:%S");
            let fill_msg = format!(
                "📊 FILL: {}|{}|{:.6}|{:.2}|{:.2}|{}|{}",
                symbol, side, quantity, price, pnl, timestamp, fill_id
            );
            let _ = tx.send(fill_msg);
            debug!("📊 Fill enviado al dashboard: {}", fill_id);
        }
    }
    
    /// Enviar mensaje de prueba
    pub async fn send_test_message(&self) -> Result<(), String> {
        let msg = "🔔 <b>PRUEBA DE CONEXIÓN</b>\nSi ves esto, Telegram funciona correctamente.";
        self.send_message(msg).await
    }
    
    // =============================================
    // VERIFICACIONES - RESPETAN LA CONFIGURACIÓN DEL .ENV
    // =============================================
    
    pub async fn is_enabled(&self) -> bool {
        *self.enabled.lock().await
    }
    
    /// Verificar si debe notificar órdenes (SEGÚN .env)
    pub async fn should_notify_order(&self) -> bool {
        let enabled = self.enabled.lock().await;
        let notify = self.notify_orders.lock().await;
        *enabled && *notify
    }
    
    /// Verificar si debe notificar fills (SEGÚN .env)
    pub async fn should_notify_fill(&self) -> bool {
        let enabled = self.enabled.lock().await;
        let notify = self.notify_fills.lock().await;
        *enabled && *notify
    }
    
    /// Verificar si debe notificar estado (SEGÚN .env)
    pub async fn should_notify_status(&self) -> bool {
        let enabled = self.enabled.lock().await;
        let notify = self.notify_status.lock().await;
        *enabled && *notify
    }
    
    // =============================================
    // FORMATEADORES DE MENSAJES
    // =============================================
    
    /// Formatear mensaje de orden colocada
    pub fn format_order(&self, symbol: &str, side: &str, quantity: f64, price: f64) -> String {
        let emoji = if side == "COMPRA" { "🟢" } else { "🔴" };
        format!(
            "{} <b>ORDEN COLOCADA</b>\n\
             Activo: {}\n\
             Tipo: {}\n\
             Cantidad: {:.6}\n\
             Precio: ${:.2}\n\
             Total: ${:.2}",
            emoji, symbol, side, quantity, price, quantity * price
        )
    }
    
    /// Formatear mensaje de fill ejecutado
    pub fn format_fill(&self, symbol: &str, side: &str, quantity: f64, price: f64, pnl: f64) -> String {
        let emoji = if pnl >= 0.0 { "✅" } else { "❌" };
        let pnl_emoji = if pnl >= 0.0 { "📈" } else { "📉" };
        let pnl_sign = if pnl >= 0.0 { "+" } else { "-" };
        
        format!(
            "{} <b>FILL EJECUTADO</b>\n\
             Activo: {}\n\
             Tipo: {}\n\
             Cantidad: {:.6}\n\
             Precio: ${:.2}\n\
             Total: ${:.2}\n\
             {} P&L: {}{:.2} USD",
            emoji, symbol, side, quantity, price, quantity * price,
            pnl_emoji, pnl_sign, pnl.abs()
        )
    }
    
    /// Formatear mensaje de estado del bot
    pub fn format_status(&self, balance: f64, pnl: f64, win_rate: f64, drawdown: f64) -> String {
        let pnl_emoji = if pnl >= 0.0 { "📈" } else { "📉" };
        let pnl_sign = if pnl >= 0.0 { "+" } else { "-" };
        let pnl_percent = if balance - pnl > 0.0 { (pnl / (balance - pnl)) * 100.0 } else { 0.0 };
        
        format!(
            "🤖 <b>ESTADO DEL BOT</b>\n\n\
             💰 Balance: ${:.2}\n\
             {} P&L Total: {}{:.2} ({:.2}%)\n\
             🎯 Win Rate: {:.1}%\n\
             📉 Drawdown: {:.2}%\n\
             ⏰ {}",
            balance,
            pnl_emoji, pnl_sign, pnl.abs(), pnl_percent,
            win_rate * 100.0,
            drawdown * 100.0,
            chrono::Local::now().format("%H:%M:%S")
        )
    }
    
    /// Formatear mensaje de alerta
    pub fn format_alert(&self, alert_type: &str, message: &str) -> String {
        let emoji = match alert_type {
            "warning" => "⚠️",
            "error" => "🚨",
            "success" => "✅",
            "info" => "ℹ️",
            _ => "🔔",
        };
        
        format!("{} <b>{}</b>\n{}", emoji, alert_type.to_uppercase(), message)
    }
}