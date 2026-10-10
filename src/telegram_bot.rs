//! Bot de Telegram real para múltiples usuarios

use reqwest::Client;
use serde_json::json;
use serde::{Serialize, Deserialize};
use tracing::{info, error, debug, warn};
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{self, Duration};
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use chrono::{Utc, DateTime};
use std::fs;
use std::path::PathBuf;

use crate::risk::RiskManager;
use crate::bot::TradingBot;
use crate::notifier::TelegramNotifier;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelegramUser {
    pub chat_id: String,
    pub username: String,
    pub first_name: String,
    pub subscribed: bool,
    pub authorized: bool,
    pub last_interaction: DateTime<Utc>,
    #[serde(default)]
    pub is_admin: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelegramStats {
    pub total_users: usize,
    pub authorized_users: usize,
    pub subscribed_users: usize,
    pub active_today: usize,
}

#[derive(Clone)]
pub struct TelegramBot {
    bot_token: String,
    client: Client,
    risk_manager: Arc<tokio::sync::Mutex<RiskManager>>,
    _bot: Arc<tokio::sync::Mutex<TradingBot>>,
    notifier: Arc<TelegramNotifier>,
    users: Arc<Mutex<Vec<TelegramUser>>>,
    admin_ids: Arc<Mutex<Vec<String>>>,
    subscribers_file: PathBuf,
}

impl TelegramBot {
    pub fn new(
        bot_token: &str,
        risk_manager: Arc<tokio::sync::Mutex<RiskManager>>,
        bot: Arc<tokio::sync::Mutex<TradingBot>>,
        notifier: Arc<TelegramNotifier>,
    ) -> Self {
        let subscribers_file = std::env::current_dir()
            .unwrap_or_default()
            .join("subscribers.json");
        
        info!("📁 Archivo de suscriptores: {:?}", subscribers_file);
        
        // Cargar usuarios de forma síncrona al iniciar
        let users = Self::load_users_sync(&subscribers_file);
        let admin_ids: Vec<String> = users.iter()
            .filter(|u| u.is_admin)
            .map(|u| u.chat_id.clone())
            .collect();
        
        let bot_instance = Self {
            bot_token: bot_token.to_string(),
            client: Client::new(),
            risk_manager,
            _bot: bot,
            notifier,
            users: Arc::new(Mutex::new(users)),
            admin_ids: Arc::new(Mutex::new(admin_ids)),
            subscribers_file,
        };
        
        bot_instance
    }
    
    fn load_users_sync(file_path: &PathBuf) -> Vec<TelegramUser> {
        info!("📂 Cargando usuarios desde archivo (sync)...");
        
        if let Ok(content) = fs::read_to_string(file_path) {
            // Intentar parsear como array de objetos (nuevo formato)
            if let Ok(users) = serde_json::from_str::<Vec<TelegramUser>>(&content) {
                info!("✅ Cargados {} usuarios desde archivo", users.len());
                return users;
            }
            
            // Intentar parsear como array de strings (formato antiguo)
            if let Ok(chat_ids) = serde_json::from_str::<Vec<String>>(&content) {
                let users: Vec<TelegramUser> = chat_ids.iter().map(|chat_id| {
                    TelegramUser {
                        chat_id: chat_id.clone(),
                        username: format!("user_{}", chat_id),
                        first_name: "Usuario".to_string(),
                        subscribed: true,
                        authorized: true,
                        last_interaction: Utc::now(),
                        is_admin: false,
                    }
                }).collect();
                info!("✅ Cargados {} usuarios desde archivo (formato legacy)", users.len());
                return users;
            }
            
            warn!("⚠️ No se pudo parsear el archivo subscribers.json");
        } else {
            info!("📂 Archivo subscribers.json no encontrado");
        }
        
        Vec::new()
    }
    
    // =============================================
    // CARGA DE USUARIOS DESDE ARCHIVO
    // =============================================
    
    async fn load_users_from_file(&self) {
        info!("📂 Cargando usuarios desde archivo...");
        
        if let Ok(content) = fs::read_to_string(&self.subscribers_file) {
            // Intentar parsear como array de objetos (nuevo formato)
            if let Ok(users) = serde_json::from_str::<Vec<TelegramUser>>(&content) {
                let mut users_lock = self.users.lock().await;
                *users_lock = users;
                info!("✅ Cargados {} usuarios desde archivo (formato completo)", users_lock.len());
                
                // Actualizar admins
                let mut admins = self.admin_ids.lock().await;
                for user in users_lock.iter() {
                    if user.is_admin {
                        admins.push(user.chat_id.clone());
                    }
                }
                return;
            }
            
            // Intentar parsear como array de strings (formato antiguo)
            if let Ok(chat_ids) = serde_json::from_str::<Vec<String>>(&content) {
                let mut users_lock = self.users.lock().await;
                for chat_id in chat_ids {
                    if !users_lock.iter().any(|u| u.chat_id == chat_id) {
                        users_lock.push(TelegramUser {
                            chat_id: chat_id.clone(),
                            username: format!("user_{}", chat_id),
                            first_name: "Usuario".to_string(),
                            subscribed: true,
                            authorized: true,
                            last_interaction: Utc::now(),
                            is_admin: false,
                        });
                    }
                }
                info!("✅ Cargados {} usuarios desde archivo (formato legacy)", users_lock.len());
                self.save_users_to_file().await;
                return;
            }
            
            warn!("⚠️ No se pudo parsear el archivo subscribers.json");
        } else {
            info!("📂 Archivo subscribers.json no encontrado, se creará automáticamente");
            self.save_users_to_file().await;
        }
    }
    
    async fn save_users_to_file(&self) {
        let users = self.users.lock().await;
        if let Ok(content) = serde_json::to_string_pretty(&*users) {
            if let Err(e) = fs::write(&self.subscribers_file, content) {
                error!("❌ Error guardando usuarios en archivo: {}", e);
            } else {
                debug!("💾 Usuarios guardados en archivo");
            }
        }
    }
    
    async fn sync_user_to_file(&self, _chat_id: &str) {
        let users = self.users.lock().await;
        if let Ok(content) = serde_json::to_string_pretty(&*users) {
            let _ = fs::write(&self.subscribers_file, content);
        }
    }
    
    // =============================================
    // API PÚBLICA PARA EL DASHBOARD
    // =============================================

    pub async fn get_users(&self) -> Vec<TelegramUser> {
        let users = self.users.lock().await;
        users.clone()
    }

    pub async fn get_stats(&self) -> TelegramStats {
        let users = self.users.lock().await;
        let now = Utc::now();
        
        TelegramStats {
            total_users: users.len(),
            authorized_users: users.iter().filter(|u| u.authorized).count(),
            subscribed_users: users.iter().filter(|u| u.subscribed).count(),
            active_today: users.iter()
                .filter(|u| (now - u.last_interaction).num_hours() < 24)
                .count(),
        }
    }

    pub async fn authorize_user(&self, chat_id: &str) -> Result<(), String> {
        let mut users = self.users.lock().await;
        if let Some(user) = users.iter_mut().find(|u| u.chat_id == chat_id) {
            user.authorized = true;
            drop(users);
            self.sync_user_to_file(chat_id).await;
            info!("✅ Usuario autorizado: {}", chat_id);
            Ok(())
        } else {
            Err("Usuario no encontrado".to_string())
        }
    }

    pub async fn deauthorize_user(&self, chat_id: &str) -> Result<(), String> {
        let mut users = self.users.lock().await;
        if let Some(user) = users.iter_mut().find(|u| u.chat_id == chat_id) {
            user.authorized = false;
            user.subscribed = false;
            drop(users);
            self.sync_user_to_file(chat_id).await;
            info!("❌ Usuario desautorizado: {}", chat_id);
            Ok(())
        } else {
            Err("Usuario no encontrado".to_string())
        }
    }

    pub async fn subscribe_user(&self, chat_id: &str) -> Result<(), String> {
        let mut users = self.users.lock().await;
        if let Some(user) = users.iter_mut().find(|u| u.chat_id == chat_id) {
            if !user.authorized {
                return Err("Usuario debe estar autorizado primero".to_string());
            }
            user.subscribed = true;
            drop(users);
            self.sync_user_to_file(chat_id).await;
            info!("🔔 Usuario suscrito: {}", chat_id);
            Ok(())
        } else {
            Err("Usuario no encontrado".to_string())
        }
    }

    pub async fn unsubscribe_user(&self, chat_id: &str) -> Result<(), String> {
        let mut users = self.users.lock().await;
        if let Some(user) = users.iter_mut().find(|u| u.chat_id == chat_id) {
            user.subscribed = false;
            drop(users);
            self.sync_user_to_file(chat_id).await;
            info!("🔕 Usuario desuscrito: {}", chat_id);
            Ok(())
        } else {
            Err("Usuario no encontrado".to_string())
        }
    }

    pub async fn delete_user(&self, chat_id: &str) -> Result<(), String> {
        let mut users = self.users.lock().await;
        let initial_len = users.len();
        users.retain(|u| u.chat_id != chat_id);
        if users.len() < initial_len {
            drop(users);
            self.sync_user_to_file(chat_id).await;
            info!("🗑️ Usuario eliminado: {}", chat_id);
            Ok(())
        } else {
            Err("Usuario no encontrado".to_string())
        }
    }

    pub async fn broadcast_message(&self, message: &str) -> Result<usize, String> {
        let users = self.users.lock().await;
        let subscribed: Vec<String> = users
            .iter()
            .filter(|u| u.subscribed && u.authorized)
            .map(|u| u.chat_id.clone())
            .collect();
        
        if subscribed.is_empty() {
            return Ok(0);
        }

        let mut sent = 0;
        for chat_id in subscribed {
            if self.send_message_to_chat(&chat_id, message).await.is_ok() {
                sent += 1;
            }
        }
        
        Ok(sent)
    }

    // =============================================
    // MÉTODOS DE ADMINISTRACIÓN
    // =============================================

    pub async fn add_admin(&self, chat_id: &str) {
        let mut admins = self.admin_ids.lock().await;
        if !admins.contains(&chat_id.to_string()) {
            admins.push(chat_id.to_string());
            info!("👑 Nuevo administrador: {}", chat_id);
        }
        
        let mut users = self.users.lock().await;
        if let Some(user) = users.iter_mut().find(|u| u.chat_id == chat_id) {
            user.is_admin = true;
        }
        drop(users);
        self.sync_user_to_file(chat_id).await;
    }

    async fn is_admin(&self, chat_id: &str) -> bool {
        let admins = self.admin_ids.lock().await;
        if admins.contains(&chat_id.to_string()) {
            return true;
        }
        drop(admins);
        
        let users = self.users.lock().await;
        users.iter().any(|u| u.chat_id == chat_id && u.is_admin)
    }

    async fn get_or_create_user(&self, chat_id: &str, username: &str, first_name: &str) -> TelegramUser {
        let user_exists = {
            let users = self.users.lock().await;
            users.iter().find(|u| u.chat_id == chat_id).cloned()
        };
        
        if let Some(mut user) = user_exists {
            user.last_interaction = Utc::now();
            user.username = username.to_string();
            user.first_name = first_name.to_string();
            
            {
                let mut users = self.users.lock().await;
                if let Some(u) = users.iter_mut().find(|u| u.chat_id == chat_id) {
                    u.last_interaction = user.last_interaction;
                    u.username = user.username.clone();
                    u.first_name = user.first_name.clone();
                }
            }
            
            return user;
        }
        
        let is_admin = chat_id == "1974934581";
        
        let new_user = TelegramUser {
            chat_id: chat_id.to_string(),
            username: username.to_string(),
            first_name: first_name.to_string(),
            subscribed: false,
            authorized: is_admin,
            last_interaction: Utc::now(),
            is_admin,
        };
        
        {
            let mut users = self.users.lock().await;
            users.push(new_user.clone());
        }
        
        self.sync_user_to_file(chat_id).await;
        info!("👤 Nuevo usuario registrado: {} (@{})", chat_id, username);
        new_user
    }

    // =============================================
    // COMANDOS DEL BOT
    // =============================================

    async fn cmd_start(&self, user: &TelegramUser) -> String {
        format!(
            "👋 <b>Bienvenido, {}!</b>\n\n\
             Tu ID: <code>{}</code>\n\
             Usuario: @{}\n\n\
             📌 <b>Estado:</b> {}\n\
             🔔 <b>Suscripción:</b> {}\n\n\
             Usa /help para ver los comandos disponibles.",
            user.first_name,
            user.chat_id,
            user.username,
            if user.authorized { "✅ Autorizado" } else { "⏳ Pendiente de autorización" },
            if user.subscribed { "✅ Activa" } else { "❌ Inactiva" }
        )
    }

    async fn cmd_help(&self, user: &TelegramUser) -> String {
        let mut help = String::from("📋 <b>COMANDOS DISPONIBLES</b>\n\n");
        
        help.push_str("<b>Generales:</b>\n");
        help.push_str("/start - Iniciar bot\n");
        help.push_str("/help - Mostrar esta ayuda\n");
        help.push_str("/subscribe - Suscribirse a notificaciones\n");
        help.push_str("/unsubscribe - Darse de baja\n\n");
        
        if user.authorized {
            help.push_str("<b>Información (requiere autorización):</b>\n");
            help.push_str("/status - Estado del bot\n");
            help.push_str("/balance - Ver balance\n");
            help.push_str("/trades - Últimos trades\n\n");
        } else {
            help.push_str("⏳ Espera a que un administrador te autorice para más comandos.\n\n");
        }
        
        if self.is_admin(&user.chat_id).await {
            help.push_str("<b>Administración:</b>\n");
            help.push_str("/users - Listar usuarios\n");
            help.push_str("/authorize &lt;id&gt; - Autorizar usuario\n");
            help.push_str("/deauthorize &lt;id&gt; - Desautorizar usuario\n");
            help.push_str("/broadcast &lt;msg&gt; - Enviar a todos\n");
        }
        
        help
    }

    async fn cmd_status(&self) -> String {
        let risk = self.risk_manager.lock().await;
        let balance = risk.current_balance().to_f64().unwrap_or(0.0);
        let pnl = risk.total_pnl().to_f64().unwrap_or(0.0);
        let win_rate = risk.win_rate() * 100.0;
        let drawdown = risk.current_drawdown().to_f64().unwrap_or(0.0) * 100.0;
        let winning = risk.winning_trades();
        let losing = risk.losing_trades();
        let total = winning + losing;
        drop(risk);
        
        format!(
            "🤖 <b>ESTADO DEL BOT</b>\n\n\
             💰 Balance: ${:.2}\n\
             📈 P&L: ${:.2} ({:.1}%)\n\
             🎯 Win Rate: {:.1}% ({}G/{}P)\n\
             📉 Drawdown: {:.2}%\n\
             🔄 Total trades: {}",
            balance, pnl, (pnl / 10.0) * 100.0,
            win_rate, winning, losing,
            drawdown, total
        )
    }

    async fn cmd_balance(&self) -> String {
        let risk = self.risk_manager.lock().await;
        let balance = risk.current_balance().to_f64().unwrap_or(0.0);
        let pnl = risk.total_pnl().to_f64().unwrap_or(0.0);
        let initial = risk.initial_capital().to_f64().unwrap_or(50.0);
        drop(risk);
        
        format!(
            "💰 <b>BALANCE</b>\n\n\
             Inicial: ${:.2}\n\
             Actual: ${:.2}\n\
             P&L: ${:.2} ({:.1}%)",
            initial, balance, pnl, (pnl / initial) * 100.0
        )
    }

    async fn cmd_trades(&self) -> String {
        let risk = self.risk_manager.lock().await;
        let recent = risk.recent_trades(5);
        
        if recent.is_empty() {
            drop(risk);
            return "📊 No hay trades recientes.".to_string();
        }
        
        let winning = risk.winning_trades();
        let losing = risk.losing_trades();
        let total = winning + losing;
        
        let mut msg = format!("📊 <b>ÚLTIMOS 5 TRADES</b>\n");
        msg.push_str(&format!("   Total: {}G/{}P ({} total)\n\n", winning, losing, total));
        
        for trade in recent {
            let sign = if trade.pnl >= Decimal::ZERO { "+" } else { "-" };
            msg.push_str(&format!(
                "{} {} {} @ ${:.0}  ({}{:.2})\n",
                if trade.side == crate::types::OrderSide::Buy { "🟢" } else { "🔴" },
                trade.symbol,
                trade.size.to_f64().unwrap_or(0.0),
                trade.price.to_f64().unwrap_or(0.0),
                sign,
                trade.pnl.to_f64().unwrap_or(0.0)
            ));
        }
        
        drop(risk);
        msg
    }

    async fn cmd_users(&self, admin_chat_id: &str) -> String {
        if !self.is_admin(admin_chat_id).await {
            return "⛔ Solo administradores pueden ver la lista de usuarios.".to_string();
        }
        
        let users = self.users.lock().await;
        if users.is_empty() {
            return "📭 No hay usuarios registrados.".to_string();
        }
        
        let mut msg = String::from("👥 <b>USUARIOS REGISTRADOS</b>\n\n");
        for (i, user) in users.iter().enumerate() {
            msg.push_str(&format!(
                "{}. <b>{}</b> (@{})\n",
                i + 1,
                user.first_name,
                user.username
            ));
            msg.push_str(&format!(
                "   ID: <code>{}</code>\n",
                user.chat_id
            ));
            msg.push_str(&format!(
                "   Estado: {} | {} | {}\n",
                if user.authorized { "✅ Autorizado" } else { "⛔ No autorizado" },
                if user.subscribed { "🔔 Suscrito" } else { "🔕 No suscrito" },
                user.last_interaction.format("%H:%M")
            ));
        }
        msg
    }

    async fn cmd_authorize(&self, admin_chat_id: &str, target_id: &str) -> String {
        if !self.is_admin(admin_chat_id).await {
            return "⛔ Solo administradores pueden autorizar usuarios.".to_string();
        }
        
        match self.authorize_user(target_id).await {
            Ok(_) => format!("✅ Usuario {} autorizado correctamente.", target_id),
            Err(e) => format!("❌ Error: {}", e),
        }
    }

    async fn cmd_deauthorize(&self, admin_chat_id: &str, target_id: &str) -> String {
        if !self.is_admin(admin_chat_id).await {
            return "⛔ Solo administradores pueden desautorizar usuarios.".to_string();
        }
        
        match self.deauthorize_user(target_id).await {
            Ok(_) => format!("✅ Usuario {} desautorizado.", target_id),
            Err(e) => format!("❌ Error: {}", e),
        }
    }

    async fn cmd_subscribe(&self, chat_id: &str) -> String {
        let user_exists = {
            let users = self.users.lock().await;
            users.iter().find(|u| u.chat_id == chat_id).cloned()
        };
        
        match user_exists {
            Some(user) => {
                if !user.authorized {
                    return "⛔ Necesitas autorización para suscribirte.".to_string();
                }
                if user.subscribed {
                    return format!("⚠️ Ya estás suscrito, {}!", user.first_name);
                }
                
                {
                    let mut users = self.users.lock().await;
                    if let Some(u) = users.iter_mut().find(|u| u.chat_id == chat_id) {
                        u.subscribed = true;
                    }
                }
                
                self.sync_user_to_file(chat_id).await;
                info!("📱 Nuevo suscriptor: {}", chat_id);
                format!("✅ Te has suscrito correctamente, {}!", user.first_name)
            }
            None => "❌ Usuario no encontrado. Usa /start primero.".to_string(),
        }
    }

    async fn cmd_unsubscribe(&self, chat_id: &str) -> String {
        let user_exists = {
            let users = self.users.lock().await;
            users.iter().find(|u| u.chat_id == chat_id).cloned()
        };
        
        match user_exists {
            Some(user) => {
                if !user.subscribed {
                    return format!("⚠️ No estabas suscrito, {}!", user.first_name);
                }
                
                {
                    let mut users = self.users.lock().await;
                    if let Some(u) = users.iter_mut().find(|u| u.chat_id == chat_id) {
                        u.subscribed = false;
                    }
                }
                
                self.sync_user_to_file(chat_id).await;
                info!("📱 Usuario desuscrito: {}", chat_id);
                format!("✅ Te has desuscrito, {}!", user.first_name)
            }
            None => "❌ Usuario no encontrado.".to_string(),
        }
    }

    async fn cmd_broadcast(&self, admin_chat_id: &str, message: &str) -> String {
        if !self.is_admin(admin_chat_id).await {
            return "⛔ Solo administradores pueden enviar broadcasts.".to_string();
        }
        
        match self.broadcast_message(message).await {
            Ok(sent) => format!("✅ Mensaje enviado a {} usuarios.", sent),
            Err(e) => format!("❌ Error: {}", e),
        }
    }

    async fn cmd_unknown(&self) -> String {
        "❓ Comando no reconocido. Usa /help para ver los comandos disponibles.".to_string()
    }

    // =============================================
    // PROCESAMIENTO DE MENSAJES
    // =============================================

    async fn process_message(&self, message: &serde_json::Value) -> Result<(), String> {
        let chat_id = message["chat"]["id"].as_i64().unwrap_or(0).to_string();
        let text = message["text"].as_str().unwrap_or("").to_string();
        let username = message["from"]["username"].as_str().unwrap_or("sin_username");
        let first_name = message["from"]["first_name"].as_str().unwrap_or("Usuario");
        
        info!("📨 Mensaje de @{} ({}): {}", username, chat_id, text);
        
        let user = self.get_or_create_user(&chat_id, username, first_name).await;
        
        let response = match text.as_str() {
            "/start" => self.cmd_start(&user).await,
            "/help" => self.cmd_help(&user).await,
            "/status" => {
                if user.authorized {
                    self.cmd_status().await
                } else {
                    "⛔ No autorizado. Espera a que un administrador te autorice.".to_string()
                }
            },
            "/balance" => {
                if user.authorized {
                    self.cmd_balance().await
                } else {
                    "⛔ No autorizado.".to_string()
                }
            },
            "/trades" => {
                if user.authorized {
                    self.cmd_trades().await
                } else {
                    "⛔ No autorizado.".to_string()
                }
            },
            "/users" => self.cmd_users(&chat_id).await,
            "/subscribe" => self.cmd_subscribe(&chat_id).await,
            "/unsubscribe" => self.cmd_unsubscribe(&chat_id).await,
            cmd if cmd.starts_with("/authorize ") => {
                let target = cmd.trim_start_matches("/authorize ");
                self.cmd_authorize(&chat_id, target).await
            },
            cmd if cmd.starts_with("/deauthorize ") => {
                let target = cmd.trim_start_matches("/deauthorize ");
                self.cmd_deauthorize(&chat_id, target).await
            },
            cmd if cmd.starts_with("/broadcast ") => {
                let msg = cmd.trim_start_matches("/broadcast ");
                self.cmd_broadcast(&chat_id, msg).await
            },
            _ => self.cmd_unknown().await,
        };
        
        self.send_message_to_chat(&chat_id, &response).await?;
        Ok(())
    }

    async fn send_message_to_chat(&self, chat_id: &str, message: &str) -> Result<(), String> {
        let url = format!("https://api.telegram.org/bot{}/sendMessage", self.bot_token);
        
        let payload = json!({
            "chat_id": chat_id,
            "text": message,
            "parse_mode": "HTML",
        });
        
        match self.client.post(&url).json(&payload).send().await {
            Ok(response) => {
                if response.status().is_success() {
                    debug!("✅ Mensaje enviado a {}", chat_id);
                    Ok(())
                } else {
                    let status = response.status();
                    let text = response.text().await.unwrap_or_default();
                    error!("❌ Error Telegram ({}): {}", status, text);
                    Err(format!("Error {}: {}", status, text))
                }
            }
            Err(e) => {
                error!("❌ Error de conexión: {}", e);
                Err(e.to_string())
            }
        }
    }

    async fn get_updates(&self, offset: &mut i64) -> Result<Vec<serde_json::Value>, String> {
        let url = format!("https://api.telegram.org/bot{}/getUpdates", self.bot_token);
        
        let params = json!({
            "offset": offset,
            "timeout": 30,
            "allowed_updates": ["message"]
        });
        
        match self.client.post(&url).json(&params).send().await {
            Ok(response) => {
                if response.status().is_success() {
                    let data: serde_json::Value = response.json().await.map_err(|e| e.to_string())?;
                    if let Some(result) = data["result"].as_array() {
                        if let Some(last) = result.last() {
                            if let Some(update_id) = last["update_id"].as_i64() {
                                *offset = update_id + 1;
                            }
                        }
                        Ok(result.clone())
                    } else {
                        Ok(vec![])
                    }
                } else {
                    let status = response.status();
                    let text = response.text().await.unwrap_or_default();
                    Err(format!("Error {}: {}", status, text))
                }
            }
            Err(e) => {
                Err(e.to_string())
            }
        }
    }

    // =============================================
    // LOOP PRINCIPAL
    // =============================================

    pub async fn run(&self) {
        info!("🤖 Bot de Telegram real iniciado. Esperando comandos...");
        
        let mut offset = 0;
        let mut interval = time::interval(Duration::from_secs(2));
        
        loop {
            interval.tick().await;
            
            match self.get_updates(&mut offset).await {
                Ok(updates) => {
                    if !updates.is_empty() {
                        debug!("📨 Recibidas {} actualizaciones", updates.len());
                    }
                    for update in updates {
                        if let Some(message) = update.get("message") {
                            if let Err(e) = self.process_message(message).await {
                                error!("Error procesando mensaje: {}", e);
                            }
                        }
                    }
                }
                Err(e) => {
                    error!("Error obteniendo updates: {}", e);
                }
            }
        }
    }
}