//! Bot de Telegram multiusuario con planes Free / Premium.
//!
//! - Premium se paga con Telegram Stars (suscripción mensual con renovación automática).
//! - Las señales de la estrategia llegan a Premium al instante; Free recibe un aviso
//!   inmediato y la señal completa con retraso (FREE_SIGNAL_DELAY_HOURS).
//! - Prueba gratis (/trial), referidos (/referral), historial público (/performance).
//! - Comandos con datos de la cuenta del bot (/status, /balance, /trades): solo admins.

use chrono::{DateTime, Duration as ChronoDuration, TimeZone, Utc};
use reqwest::Client;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{self, Duration};
use tracing::{debug, error, info, warn};

use crate::bot::TradingBot;
use crate::notifier::TelegramNotifier;
use crate::risk::RiskManager;
use crate::signals::{Performance, SignalEvent, SignalHistory, SignalRecord, DISCLAIMER, STRATEGY_REBOUND, STRATEGY_TREND};

/// Duración de una suscripción de Telegram Stars (fija por Telegram: 30 días)
const STARS_SUBSCRIPTION_PERIOD_SECS: i64 = 2_592_000;
/// Pausa entre envíos masivos (límite de Telegram ~30 mensajes/s)
const BULK_SEND_PAUSE_MS: u64 = 40;

fn now() -> DateTime<Utc> {
    Utc::now()
}

fn env_or<T: std::str::FromStr>(key: &str, default: T) -> T {
    std::env::var(key).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

/// Escapa texto de usuario para parse_mode HTML
fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

// =============================================
// MODELOS
// =============================================

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
    /// Fin del acceso Premium (pagado, prueba o regalo)
    #[serde(default)]
    pub premium_until: Option<DateTime<Utc>>,
    #[serde(default)]
    pub trial_used: bool,
    #[serde(default)]
    pub referred_by: Option<String>,
    #[serde(default)]
    pub referral_count: u32,
    /// El usuario bloqueó al bot (Telegram devuelve 403)
    #[serde(default)]
    pub blocked: bool,
    #[serde(default)]
    pub total_paid_stars: u64,
    #[serde(default)]
    pub expiry_reminded: bool,
    #[serde(default)]
    pub expired_notified: bool,
    #[serde(default = "now")]
    pub joined_at: DateTime<Utc>,
    /// Calculado al consultar (para el dashboard)
    #[serde(default, skip_deserializing)]
    pub is_premium: bool,
}

impl TelegramUser {
    fn new(chat_id: &str, username: &str, first_name: &str, authorized: bool, is_admin: bool) -> Self {
        Self {
            chat_id: chat_id.to_string(),
            username: username.to_string(),
            first_name: first_name.to_string(),
            subscribed: false,
            authorized,
            last_interaction: Utc::now(),
            is_admin,
            premium_until: None,
            trial_used: false,
            referred_by: None,
            referral_count: 0,
            blocked: false,
            total_paid_stars: 0,
            expiry_reminded: false,
            expired_notified: false,
            joined_at: Utc::now(),
            is_premium: false,
        }
    }

    pub fn premium_active(&self) -> bool {
        self.premium_until.map_or(false, |t| t > Utc::now())
    }

    /// Extiende Premium `days` días desde hoy o desde el vencimiento actual, lo que sea mayor
    fn extend_premium(&mut self, days: i64) -> DateTime<Utc> {
        let base = self.premium_until.filter(|t| *t > Utc::now()).unwrap_or_else(Utc::now);
        let until = base + ChronoDuration::days(days);
        self.premium_until = Some(until);
        self.expiry_reminded = false;
        self.expired_notified = false;
        until
    }

    fn receives_alerts(&self) -> bool {
        self.authorized && self.subscribed && !self.blocked
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TelegramStats {
    pub total_users: usize,
    pub authorized_users: usize,
    pub subscribed_users: usize,
    pub active_today: usize,
    #[serde(default)]
    pub premium_users: usize,
    #[serde(default)]
    pub paying_users: usize,
    #[serde(default)]
    pub blocked_users: usize,
    #[serde(default)]
    pub new_users_7d: usize,
    #[serde(default)]
    pub revenue_stars_30d: u64,
    #[serde(default)]
    pub revenue_stars_total: u64,
    #[serde(default)]
    pub premium_price_stars: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentRecord {
    pub chat_id: String,
    pub stars: u64,
    pub charge_id: String,
    pub payload: String,
    pub recurring: bool,
    pub at: DateTime<Utc>,
    pub premium_until: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub struct MonetizationConfig {
    pub premium_price_stars: u32,
    pub premium_days: i64,
    pub trial_days: i64,
    pub referral_bonus_days: i64,
    pub free_signal_delay_hours: i64,
    pub auto_authorize: bool,
    pub support_contact: String,
}

impl MonetizationConfig {
    pub fn from_env() -> Self {
        Self {
            premium_price_stars: env_or("PREMIUM_PRICE_STARS", 500),
            premium_days: env_or("PREMIUM_DAYS", 30),
            trial_days: env_or("TRIAL_DAYS", 3),
            referral_bonus_days: env_or("REFERRAL_BONUS_DAYS", 7),
            free_signal_delay_hours: env_or("FREE_SIGNAL_DELAY_HOURS", 24),
            auto_authorize: env_or("TELEGRAM_AUTO_AUTHORIZE", true),
            support_contact: std::env::var("PAYSUPPORT_CONTACT").unwrap_or_else(|_| "el administrador".to_string()),
        }
    }
}

// =============================================
// BOT
// =============================================

#[derive(Clone)]
pub struct TelegramBot {
    bot_token: String,
    client: Client,
    risk_manager: Arc<Mutex<RiskManager>>,
    _bot: Arc<Mutex<TradingBot>>,
    notifier: Arc<TelegramNotifier>,
    users: Arc<Mutex<Vec<TelegramUser>>>,
    admin_ids: Arc<Mutex<Vec<String>>>,
    subscribers_file: PathBuf,
    payments: Arc<Mutex<Vec<PaymentRecord>>>,
    payments_file: PathBuf,
    history: Arc<Mutex<SignalHistory>>,
    bot_username: Arc<Mutex<String>>,
    cfg: MonetizationConfig,
}

impl TelegramBot {
    pub fn new(
        bot_token: &str,
        risk_manager: Arc<Mutex<RiskManager>>,
        bot: Arc<Mutex<TradingBot>>,
        notifier: Arc<TelegramNotifier>,
        history: Arc<Mutex<SignalHistory>>,
    ) -> Self {
        let data_dir = std::env::var("DATA_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| std::env::current_dir().unwrap_or_default());
        let _ = fs::create_dir_all(&data_dir);
        let subscribers_file = data_dir.join("subscribers.json");
        // Migración: si los datos estaban en el directorio actual, copiarlos a DATA_DIR
        let legacy = std::env::current_dir().unwrap_or_default().join("subscribers.json");
        if !subscribers_file.exists() && legacy.exists() && legacy != subscribers_file {
            match fs::copy(&legacy, &subscribers_file) {
                Ok(_) => info!("📦 subscribers.json migrado a {:?}", subscribers_file),
                Err(e) => error!("❌ No se pudo migrar subscribers.json: {}", e),
            }
        }
        let payments_file = data_dir.join("payments.json");
        info!("📁 Datos de Telegram en {:?}", data_dir);

        let users = Self::load_users_sync(&subscribers_file);
        let admin_ids: Vec<String> = users.iter().filter(|u| u.is_admin).map(|u| u.chat_id.clone()).collect();
        let payments: Vec<PaymentRecord> = fs::read_to_string(&payments_file)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        let cfg = MonetizationConfig::from_env();
        info!(
            "💎 Premium: {} ⭐ / {} días | prueba {} días | referido +{} días | retraso free {} h",
            cfg.premium_price_stars, cfg.premium_days, cfg.trial_days, cfg.referral_bonus_days, cfg.free_signal_delay_hours
        );

        Self {
            bot_token: bot_token.to_string(),
            client: Client::new(),
            risk_manager,
            _bot: bot,
            notifier,
            users: Arc::new(Mutex::new(users)),
            admin_ids: Arc::new(Mutex::new(admin_ids)),
            subscribers_file,
            payments: Arc::new(Mutex::new(payments)),
            payments_file,
            history,
            bot_username: Arc::new(Mutex::new(String::new())),
            cfg,
        }
    }

    fn load_users_sync(file_path: &PathBuf) -> Vec<TelegramUser> {
        let content = match fs::read_to_string(file_path) {
            Ok(c) => c,
            Err(_) => {
                info!("📂 {:?} no existe; se creará", file_path);
                return Vec::new();
            }
        };
        if let Ok(users) = serde_json::from_str::<Vec<TelegramUser>>(&content) {
            info!("✅ Cargados {} usuarios", users.len());
            return users;
        }
        // Formato antiguo: lista de chat_ids
        if let Ok(chat_ids) = serde_json::from_str::<Vec<String>>(&content) {
            let users: Vec<TelegramUser> = chat_ids
                .iter()
                .map(|id| {
                    let mut u = TelegramUser::new(id, &format!("user_{}", id), "Usuario", true, false);
                    u.subscribed = true;
                    u
                })
                .collect();
            info!("✅ Cargados {} usuarios (formato legacy)", users.len());
            return users;
        }
        error!("❌ No se pudo parsear {:?}; se inicia vacío (el archivo NO se sobrescribe hasta el primer cambio)", file_path);
        Vec::new()
    }

    async fn save_users(&self) {
        let users = self.users.lock().await;
        match serde_json::to_string_pretty(&*users) {
            Ok(content) => {
                if let Err(e) = fs::write(&self.subscribers_file, content) {
                    error!("❌ Error guardando usuarios: {}", e);
                }
            }
            Err(e) => error!("❌ Error serializando usuarios: {}", e),
        }
    }

    async fn save_payments(&self) {
        let payments = self.payments.lock().await;
        if let Ok(content) = serde_json::to_string_pretty(&*payments) {
            if let Err(e) = fs::write(&self.payments_file, content) {
                error!("❌ Error guardando pagos: {}", e);
            }
        }
    }

    /// Modifica un usuario y persiste. Devuelve lo que retorne `f`, o error si no existe.
    async fn update_user<R>(&self, chat_id: &str, f: impl FnOnce(&mut TelegramUser) -> R) -> Result<R, String> {
        let r = {
            let mut users = self.users.lock().await;
            let user = users.iter_mut().find(|u| u.chat_id == chat_id).ok_or("Usuario no encontrado")?;
            f(user)
        };
        self.save_users().await;
        Ok(r)
    }

    async fn find_user(&self, chat_id: &str) -> Option<TelegramUser> {
        self.users.lock().await.iter().find(|u| u.chat_id == chat_id).cloned()
    }

    // =============================================
    // API PÚBLICA PARA EL DASHBOARD
    // =============================================

    pub async fn get_users(&self) -> Vec<TelegramUser> {
        let mut users = self.users.lock().await.clone();
        for u in users.iter_mut() {
            u.is_premium = u.premium_active();
        }
        users
    }

    pub async fn get_stats(&self) -> TelegramStats {
        let users = self.users.lock().await;
        let payments = self.payments.lock().await;
        let now = Utc::now();
        TelegramStats {
            total_users: users.len(),
            authorized_users: users.iter().filter(|u| u.authorized).count(),
            subscribed_users: users.iter().filter(|u| u.subscribed).count(),
            active_today: users.iter().filter(|u| (now - u.last_interaction).num_hours() < 24).count(),
            premium_users: users.iter().filter(|u| u.premium_active()).count(),
            paying_users: users.iter().filter(|u| u.total_paid_stars > 0 && u.premium_active()).count(),
            blocked_users: users.iter().filter(|u| u.blocked).count(),
            new_users_7d: users.iter().filter(|u| (now - u.joined_at).num_days() < 7).count(),
            revenue_stars_30d: payments.iter().filter(|p| (now - p.at).num_days() < 30).map(|p| p.stars).sum(),
            revenue_stars_total: payments.iter().map(|p| p.stars).sum(),
            premium_price_stars: self.cfg.premium_price_stars,
        }
    }

    pub async fn get_payments(&self) -> Vec<PaymentRecord> {
        let mut p = self.payments.lock().await.clone();
        p.reverse();
        p
    }

    pub async fn get_performance(&self) -> Performance {
        self.history.lock().await.performance()
    }

    /// Rendimiento separado por estrategia (para el dashboard)
    pub async fn get_performance_by_strategy(&self) -> (Performance, Performance) {
        let h = self.history.lock().await;
        (h.performance_for(Some(STRATEGY_TREND)), h.performance_for(Some(STRATEGY_REBOUND)))
    }

    /// Texto de /performance: total y luego cada estrategia
    async fn performance_text(&self) -> String {
        let h = self.history.lock().await;
        let all = h.performance();
        if all.closed == 0 && all.open == 0 {
            return all.format();
        }
        format!(
            "{}
{}
{}",
            all.format().replace(&format!("

{}", DISCLAIMER), ""),
            h.performance_for(Some(STRATEGY_TREND)).format_section("🚀 Tendencia"),
            h.performance_for(Some(STRATEGY_REBOUND)).format_section("🔄 Rebote")
        ) + "
" + DISCLAIMER
    }

    pub async fn get_signals(&self, limit: usize) -> Vec<SignalRecord> {
        let h = self.history.lock().await;
        h.records.iter().rev().take(limit).cloned().collect()
    }

    pub async fn authorize_user(&self, chat_id: &str) -> Result<(), String> {
        self.update_user(chat_id, |u| u.authorized = true).await?;
        info!("✅ Usuario autorizado: {}", chat_id);
        Ok(())
    }

    pub async fn deauthorize_user(&self, chat_id: &str) -> Result<(), String> {
        self.update_user(chat_id, |u| {
            u.authorized = false;
            u.subscribed = false;
        })
        .await?;
        info!("❌ Usuario desautorizado: {}", chat_id);
        Ok(())
    }

    pub async fn subscribe_user(&self, chat_id: &str) -> Result<(), String> {
        self.update_user(chat_id, |u| {
            if !u.authorized {
                return Err("Usuario debe estar autorizado primero".to_string());
            }
            u.subscribed = true;
            Ok(())
        })
        .await??;
        Ok(())
    }

    pub async fn unsubscribe_user(&self, chat_id: &str) -> Result<(), String> {
        self.update_user(chat_id, |u| u.subscribed = false).await
    }

    pub async fn delete_user(&self, chat_id: &str) -> Result<(), String> {
        let removed = {
            let mut users = self.users.lock().await;
            let n = users.len();
            users.retain(|u| u.chat_id != chat_id);
            users.len() < n
        };
        if !removed {
            return Err("Usuario no encontrado".to_string());
        }
        self.save_users().await;
        info!("🗑️ Usuario eliminado: {}", chat_id);
        Ok(())
    }

    /// Regala o extiende Premium (dashboard / admin)
    pub async fn grant_premium(&self, chat_id: &str, days: i64) -> Result<DateTime<Utc>, String> {
        if days <= 0 || days > 3650 {
            return Err("Días inválidos".to_string());
        }
        let until = self
            .update_user(chat_id, |u| {
                u.authorized = true;
                u.subscribed = true;
                u.extend_premium(days)
            })
            .await?;
        let _ = self
            .deliver(
                chat_id,
                &format!(
                    "🎁 <b>¡Tienes Premium!</b>\nActivo hasta el {}.\nRecibirás las señales al instante.",
                    until.format("%d-%m-%Y")
                ),
                None,
            )
            .await;
        info!("💎 Premium regalado a {} hasta {}", chat_id, until);
        Ok(until)
    }

    pub async fn revoke_premium(&self, chat_id: &str) -> Result<(), String> {
        self.update_user(chat_id, |u| {
            u.premium_until = None;
            u.expired_notified = true;
        })
        .await
    }

    pub async fn broadcast_message(&self, message: &str) -> Result<usize, String> {
        self.broadcast_segment(message, "all").await
    }

    /// Envío masivo a un segmento: "all" | "free" | "premium"
    pub async fn broadcast_segment(&self, message: &str, segment: &str) -> Result<usize, String> {
        let targets: Vec<String> = self
            .users
            .lock()
            .await
            .iter()
            .filter(|u| u.receives_alerts())
            .filter(|u| match segment {
                "premium" => u.premium_active(),
                "free" => !u.premium_active(),
                _ => true,
            })
            .map(|u| u.chat_id.clone())
            .collect();
        Ok(self.send_bulk(&targets, message).await)
    }

    pub async fn add_admin(&self, chat_id: &str) {
        {
            let mut admins = self.admin_ids.lock().await;
            if !admins.iter().any(|a| a == chat_id) {
                admins.push(chat_id.to_string());
                info!("👑 Administrador: {}", chat_id);
            }
        }
        let mut changed = false;
        {
            let mut users = self.users.lock().await;
            if let Some(user) = users.iter_mut().find(|u| u.chat_id == chat_id) {
                changed = !user.is_admin || !user.authorized;
                user.is_admin = true;
                user.authorized = true;
            }
        }
        if changed {
            self.save_users().await;
        }
    }

    async fn is_admin(&self, chat_id: &str) -> bool {
        if self.admin_ids.lock().await.iter().any(|a| a == chat_id) {
            return true;
        }
        self.users.lock().await.iter().any(|u| u.chat_id == chat_id && u.is_admin)
    }

    async fn get_or_create_user(&self, chat_id: &str, username: &str, first_name: &str) -> (TelegramUser, bool) {
        {
            let mut users = self.users.lock().await;
            if let Some(u) = users.iter_mut().find(|u| u.chat_id == chat_id) {
                u.last_interaction = Utc::now();
                u.username = username.to_string();
                u.first_name = first_name.to_string();
                u.blocked = false;
                return (u.clone(), false);
            }
        }
        let is_admin = self.admin_ids.lock().await.iter().any(|a| a == chat_id);
        let mut user = TelegramUser::new(chat_id, username, first_name, is_admin || self.cfg.auto_authorize, is_admin);
        // Quien inicia el bot recibe los avisos (puede darse de baja con /unsubscribe)
        user.subscribed = user.authorized;
        self.users.lock().await.push(user.clone());
        self.save_users().await;
        info!("👤 Nuevo usuario: {} (@{})", chat_id, username);
        (user, true)
    }

    // =============================================
    // ENVÍO
    // =============================================

    async fn api(&self, method: &str, payload: &serde_json::Value) -> Result<serde_json::Value, (u16, String)> {
        let url = format!("https://api.telegram.org/bot{}/{}", self.bot_token, method);
        // without_url: la URL incluye el token del bot y no debe aparecer en errores ni logs
        let resp = self
            .client
            .post(&url)
            .json(payload)
            .send()
            .await
            .map_err(|e| (0, e.without_url().to_string()))?;
        let status = resp.status().as_u16();
        let body: serde_json::Value = resp.json().await.map_err(|e| (status, e.without_url().to_string()))?;
        if body["ok"].as_bool() == Some(true) {
            Ok(body["result"].clone())
        } else {
            Err((status, body["description"].as_str().unwrap_or("error desconocido").to_string()))
        }
    }

    /// Envía un mensaje. Si el usuario bloqueó al bot, lo marca para no reintentar.
    async fn deliver(&self, chat_id: &str, text: &str, keyboard: Option<serde_json::Value>) -> Result<(), String> {
        let mut payload = json!({
            "chat_id": chat_id,
            "text": text,
            "parse_mode": "HTML",
            "disable_web_page_preview": true,
        });
        if let Some(k) = keyboard {
            payload["reply_markup"] = k;
        }
        match self.api("sendMessage", &payload).await {
            Ok(_) => Ok(()),
            Err((403, desc)) => {
                warn!("🚫 {} bloqueó al bot: {}", chat_id, desc);
                let _ = self.update_user(chat_id, |u| u.blocked = true).await;
                Err(desc)
            }
            Err((code, desc)) => {
                error!("❌ Telegram {} a {}: {}", code, chat_id, desc);
                Err(desc)
            }
        }
    }

    async fn send_bulk(&self, chat_ids: &[String], text: &str) -> usize {
        let mut sent = 0;
        for id in chat_ids {
            if self.deliver(id, text, None).await.is_ok() {
                sent += 1;
            }
            time::sleep(Duration::from_millis(BULK_SEND_PAUSE_MS)).await;
        }
        sent
    }

    async fn notify_admins(&self, text: &str) {
        let admins = self.admin_ids.lock().await.clone();
        for a in admins {
            let _ = self.deliver(&a, text, None).await;
        }
    }

    // =============================================
    // PAGOS (Telegram Stars)
    // =============================================

    /// Link de pago. Intenta suscripción mensual (renovación automática) y si no, pago único.
    async fn create_invoice_link(&self, chat_id: &str) -> Result<String, String> {
        let mut payload = json!({
            "title": "Señales Premium",
            "description": format!(
                "Señales de trading en tiempo real (entrada, stop y tamaño sugerido) durante {} días.",
                self.cfg.premium_days
            ),
            "payload": format!("premium:{}", chat_id),
            "currency": "XTR",
            "prices": [{ "label": "Premium", "amount": self.cfg.premium_price_stars }],
        });
        if self.cfg.premium_days == 30 {
            payload["subscription_period"] = json!(STARS_SUBSCRIPTION_PERIOD_SECS);
            match self.api("createInvoiceLink", &payload).await {
                Ok(v) => return v.as_str().map(String::from).ok_or_else(|| "respuesta inválida".to_string()),
                Err((_, e)) => warn!("Suscripción Stars no disponible ({}); se usa pago único", e),
            }
            payload.as_object_mut().map(|o| o.remove("subscription_period"));
        }
        self.api("createInvoiceLink", &payload)
            .await
            .map_err(|(_, e)| e)?
            .as_str()
            .map(String::from)
            .ok_or_else(|| "respuesta inválida".to_string())
    }

    async fn handle_pre_checkout(&self, q: &serde_json::Value) {
        let id = q["id"].as_str().unwrap_or_default();
        let valid = q["currency"].as_str() == Some("XTR")
            && q["invoice_payload"].as_str().map_or(false, |p| p.starts_with("premium:"));
        let mut payload = json!({ "pre_checkout_query_id": id, "ok": valid });
        if !valid {
            payload["error_message"] = json!("Pago no válido. Usa /premium para generar uno nuevo.");
        }
        if let Err((_, e)) = self.api("answerPreCheckoutQuery", &payload).await {
            error!("❌ answerPreCheckoutQuery: {}", e);
        }
    }

    async fn handle_successful_payment(&self, chat_id: &str, p: &serde_json::Value) {
        let stars = p["total_amount"].as_u64().unwrap_or(0);
        let charge_id = p["telegram_payment_charge_id"].as_str().unwrap_or_default().to_string();
        let recurring = p["is_recurring"].as_bool().unwrap_or(false);
        let sub_until = p["subscription_expiration_date"]
            .as_i64()
            .and_then(|ts| Utc.timestamp_opt(ts, 0).single());
        let days = self.cfg.premium_days;

        let result = self
            .update_user(chat_id, |u| {
                let first_payment = u.total_paid_stars == 0;
                u.total_paid_stars += stars;
                u.authorized = true;
                u.subscribed = true;
                let until = match sub_until {
                    Some(t) => {
                        u.premium_until = Some(t);
                        u.expiry_reminded = false;
                        u.expired_notified = false;
                        t
                    }
                    None => u.extend_premium(days),
                };
                (until, if first_payment { u.referred_by.clone() } else { None })
            })
            .await;

        let (until, referrer) = match result {
            Ok(x) => x,
            Err(e) => {
                error!("❌ Pago de usuario desconocido {}: {}", chat_id, e);
                (Utc::now(), None)
            }
        };

        self.payments.lock().await.push(PaymentRecord {
            chat_id: chat_id.to_string(),
            stars,
            charge_id: charge_id.clone(),
            payload: p["invoice_payload"].as_str().unwrap_or_default().to_string(),
            recurring,
            at: Utc::now(),
            premium_until: Some(until),
        });
        self.save_payments().await;
        info!("💰 Pago {} ⭐ de {} (recurrente: {}) charge={}", stars, chat_id, recurring, charge_id);

        let _ = self
            .deliver(
                chat_id,
                &format!(
                    "✅ <b>¡Gracias! Premium activo hasta el {}.</b>\n\n\
                     Recibirás cada señal al instante con entrada, stop y tamaño sugerido.\n\
                     /signals muestra las posiciones abiertas ahora.",
                    until.format("%d-%m-%Y")
                ),
                None,
            )
            .await;
        self.notify_admins(&format!("💰 Nuevo pago: {} ⭐ de <code>{}</code>{}", stars, chat_id, if recurring { " (renovación)" } else { "" }))
            .await;

        if let Some(ref_id) = referrer {
            let bonus = self.cfg.referral_bonus_days;
            if let Ok(until) = self
                .update_user(&ref_id, |u| {
                    u.referral_count += 1;
                    u.extend_premium(bonus)
                })
                .await
            {
                let _ = self
                    .deliver(
                        &ref_id,
                        &format!(
                            "🎉 Un amigo que invitaste se hizo Premium.\nTe regalamos {} días: Premium hasta el {}.",
                            bonus,
                            until.format("%d-%m-%Y")
                        ),
                        None,
                    )
                    .await;
            }
        }
    }

    // =============================================
    // SEÑALES
    // =============================================

    async fn handle_signal(&self, event: SignalEvent) {
        // El historial lo graba signals::spawn_recorder (funciona aunque no haya Telegram)
        let users = self.users.lock().await.clone();
        let premium: Vec<String> = users
            .iter()
            .filter(|u| u.receives_alerts() && (u.premium_active() || u.is_admin))
            .map(|u| u.chat_id.clone())
            .collect();
        let free: Vec<String> = users
            .iter()
            .filter(|u| u.receives_alerts() && !u.premium_active() && !u.is_admin)
            .map(|u| u.chat_id.clone())
            .collect();

        let sent = self.send_bulk(&premium, &event.format_premium()).await;
        info!("📡 Señal {} {} enviada a {} premium", event.strategy(), event.symbol(), sent);

        if let Some(teaser) = event.format_free_teaser(self.cfg.free_signal_delay_hours) {
            self.send_bulk(&free, &teaser).await;
        }
    }

    async fn signal_loop(&self) {
        let mut rx = self.notifier.subscribe_signals();
        loop {
            match rx.recv().await {
                Ok(ev) => self.handle_signal(ev).await,
                Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                    warn!("⚠️ Se perdieron {} señales (bot de Telegram lento)", n)
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            }
        }
    }

    /// Cada minuto: libera señales retrasadas a Free y gestiona vencimientos de Premium
    async fn maintenance_loop(&self) {
        let mut interval = time::interval(Duration::from_secs(60));
        loop {
            interval.tick().await;
            self.release_delayed_signals().await;
            self.check_expirations().await;
        }
    }

    async fn release_delayed_signals(&self) {
        let delay = ChronoDuration::hours(self.cfg.free_signal_delay_hours);
        let due: Vec<SignalRecord> = {
            let mut h = self.history.lock().await;
            let now = Utc::now();
            let mut due = Vec::new();
            for r in h.records.iter_mut() {
                if !r.free_released && r.at + delay <= now {
                    r.free_released = true;
                    // Solo entradas y cierres; los movimientos de stop no aportan con retraso
                    if !matches!(r.event, SignalEvent::StopMoved { .. }) {
                        due.push(r.clone());
                    }
                }
            }
            if !due.is_empty() {
                h.save();
            }
            due
        };
        if due.is_empty() {
            return;
        }
        let free: Vec<String> = self
            .users
            .lock()
            .await
            .iter()
            .filter(|u| u.receives_alerts() && !u.premium_active() && !u.is_admin)
            .map(|u| u.chat_id.clone())
            .collect();
        for r in due {
            let text = format!(
                "⏱️ <i>Señal de hace {} h (Premium la recibió al instante — /premium)</i>\n\n{}",
                self.cfg.free_signal_delay_hours,
                r.event.format_premium()
            );
            self.send_bulk(&free, &text).await;
        }
    }

    async fn check_expirations(&self) {
        let now = Utc::now();
        let mut to_remind = Vec::new();
        let mut to_expire = Vec::new();
        {
            let mut users = self.users.lock().await;
            for u in users.iter_mut() {
                let Some(until) = u.premium_until else { continue };
                if until > now && until - now <= ChronoDuration::days(3) && !u.expiry_reminded {
                    u.expiry_reminded = true;
                    to_remind.push((u.chat_id.clone(), until));
                } else if until <= now && !u.expired_notified {
                    u.expired_notified = true;
                    to_expire.push(u.chat_id.clone());
                }
            }
        }
        if to_remind.is_empty() && to_expire.is_empty() {
            return;
        }
        self.save_users().await;
        for (id, until) in to_remind {
            let _ = self
                .deliver(
                    &id,
                    &format!(
                        "⏳ Tu Premium vence el {}.\nSi tienes suscripción activa se renueva sola; si no, usa /premium.",
                        until.format("%d-%m-%Y")
                    ),
                    None,
                )
                .await;
        }
        for id in to_expire {
            let _ = self
                .deliver(&id, "🔒 Tu Premium terminó. Seguirás recibiendo las señales con retraso.\n👉 /premium para volver.", None)
                .await;
        }
    }

    // =============================================
    // COMANDOS
    // =============================================

    async fn cmd_start(&self, user: &TelegramUser, is_new: bool, payload: &str) -> String {
        // Referido: /start ref_<chat_id>
        if is_new {
            if let Some(ref_id) = payload.strip_prefix("ref_") {
                if ref_id != user.chat_id && self.find_user(ref_id).await.is_some() {
                    let _ = self.update_user(&user.chat_id, |u| u.referred_by = Some(ref_id.to_string())).await;
                    info!("🤝 {} referido por {}", user.chat_id, ref_id);
                }
            }
        }
        let premium = if user.premium_active() {
            format!("💎 Premium hasta el {}", user.premium_until.map(|t| t.format("%d-%m-%Y").to_string()).unwrap_or_default())
        } else {
            "🆓 Plan Free".to_string()
        };
        format!(
            "👋 <b>Hola, {}!</b>\n\n\
             Recibe señales de trading de cripto (Hyperliquid) generadas por un sistema de seguimiento de tendencia.\n\n\
             {}\n{}\n\n\
             🆓 <b>Free:</b> aviso de cada señal y detalle completo {} h después.\n\
             💎 <b>Premium:</b> señal al instante con entrada, stop y tamaño sugerido.\n\n\
             /performance — resultados de las señales\n\
             /trial — prueba Premium {} días gratis\n\
             /premium — suscribirse\n\
             /help — todos los comandos\n\n{}",
            esc(&user.first_name),
            premium,
            if user.authorized { "" } else { "⏳ Pendiente de autorización." },
            self.cfg.free_signal_delay_hours,
            self.cfg.trial_days,
            DISCLAIMER
        )
    }

    async fn cmd_help(&self, user: &TelegramUser) -> String {
        let mut help = String::from(
            "📋 <b>COMANDOS</b>\n\n\
             /premium — Planes y pago\n\
             /trial — Prueba Premium gratis\n\
             /plan — Tu plan actual\n\
             /signals — Señales abiertas (Premium)\n\
             /performance — Resultados de las señales\n\
             /referral — Invita amigos y gana días Premium\n\
             /subscribe /unsubscribe — Activar o pausar avisos\n\
             /terms — Términos\n\
             /paysupport — Ayuda con pagos\n",
        );
        if self.is_admin(&user.chat_id).await {
            help.push_str(
                "\n<b>Admin:</b>\n\
                 /status /balance /trades — Cuenta del bot\n\
                 /stats — Usuarios e ingresos\n\
                 /users — Listar usuarios\n\
                 /authorize &lt;id&gt; /deauthorize &lt;id&gt;\n\
                 /grant &lt;id&gt; &lt;días&gt; /revoke &lt;id&gt;\n\
                 /refund &lt;id&gt; &lt;charge_id&gt;\n\
                 /broadcast &lt;msg&gt; — a todos los suscritos\n\
                 /broadcast_free &lt;msg&gt; /broadcast_premium &lt;msg&gt;\n",
            );
        }
        help
    }

    async fn cmd_premium(&self, user: &TelegramUser) -> (String, Option<serde_json::Value>) {
        let status = if user.premium_active() {
            format!(
                "Ya eres Premium hasta el {}. Puedes renovar por adelantado:\n\n",
                user.premium_until.map(|t| t.format("%d-%m-%Y").to_string()).unwrap_or_default()
            )
        } else {
            String::new()
        };
        let text = format!(
            "💎 <b>PREMIUM</b>\n\n{}\
             ✅ Cada señal al instante (entrada, stop, tamaño sugerido)\n\
             ✅ Avisos para subir el stop y cerrar\n\
             ✅ /signals con las posiciones abiertas\n\n\
             Precio: <b>{} ⭐ Telegram Stars</b> cada {} días{}.\n\
             Las Stars se compran dentro de Telegram.\n\n{}",
            status,
            self.cfg.premium_price_stars,
            self.cfg.premium_days,
            if self.cfg.premium_days == 30 { " (renovación automática, cancelable desde Telegram)" } else { "" },
            DISCLAIMER
        );
        match self.create_invoice_link(&user.chat_id).await {
            Ok(link) => (
                text,
                Some(json!({ "inline_keyboard": [[{ "text": format!("⭐ Pagar {} Stars", self.cfg.premium_price_stars), "url": link }]] })),
            ),
            Err(e) => {
                error!("❌ No se pudo crear la factura: {}", e);
                (format!("{}\n\n⚠️ No se pudo generar el pago ahora. Intenta en unos minutos.", text), None)
            }
        }
    }

    async fn cmd_trial(&self, user: &TelegramUser) -> String {
        if user.trial_used {
            return "Ya usaste tu prueba gratis. 👉 /premium".to_string();
        }
        if user.premium_active() {
            return "Ya tienes Premium activo 💎".to_string();
        }
        let days = self.cfg.trial_days;
        match self
            .update_user(&user.chat_id, |u| {
                u.trial_used = true;
                u.authorized = true;
                u.subscribed = true;
                u.extend_premium(days)
            })
            .await
        {
            Ok(until) => format!(
                "🎉 <b>Prueba Premium activada</b> hasta el {}.\nRecibirás las señales al instante. /signals para ver las abiertas.",
                until.format("%d-%m-%Y %H:%M UTC")
            ),
            Err(e) => format!("❌ {}", e),
        }
    }

    async fn cmd_plan(&self, user: &TelegramUser) -> String {
        if user.premium_active() {
            format!(
                "💎 <b>Premium</b> hasta el {}\nAvisos: {}",
                user.premium_until.map(|t| t.format("%d-%m-%Y").to_string()).unwrap_or_default(),
                if user.subscribed { "activos" } else { "pausados (/subscribe)" }
            )
        } else {
            format!(
                "🆓 <b>Free</b> — señales con {} h de retraso.\nAvisos: {}\n\n/trial o /premium",
                self.cfg.free_signal_delay_hours,
                if user.subscribed { "activos" } else { "pausados (/subscribe)" }
            )
        }
    }

    async fn cmd_signals(&self, user: &TelegramUser) -> String {
        if !user.premium_active() && !user.is_admin {
            return "🔒 Las posiciones abiertas son para Premium. 👉 /premium".to_string();
        }
        // Reconstruir posiciones abiertas desde el historial
        let h = self.history.lock().await;
        let mut open: Vec<(String, Decimal, Decimal, DateTime<Utc>)> = Vec::new();
        for r in &h.records {
            match &r.event {
                SignalEvent::Entry { symbol, price, stop, .. } => {
                    open.retain(|o| &o.0 != symbol);
                    open.push((symbol.clone(), *price, *stop, r.at));
                }
                SignalEvent::StopMoved { symbol, stop, .. } => {
                    if let Some(o) = open.iter_mut().find(|o| &o.0 == symbol) {
                        o.2 = *stop;
                    }
                }
                SignalEvent::Exit { symbol, .. } => open.retain(|o| &o.0 != symbol),
            }
        }
        if open.is_empty() {
            return "📭 No hay señales abiertas ahora. Te avisaremos en la próxima.".to_string();
        }
        let mut msg = String::from("📈 <b>SEÑALES ABIERTAS</b>\n\n");
        for (symbol, price, stop, at) in open {
            msg.push_str(&format!(
                "• <b>{}</b> compra ${} — stop ${} (desde {})\n",
                symbol,
                price.round_dp(6).normalize(),
                stop.round_dp(6).normalize(),
                at.format("%d-%m %H:%M UTC")
            ));
        }
        msg
    }

    async fn cmd_referral(&self, user: &TelegramUser) -> String {
        let bot = self.bot_username.lock().await.clone();
        format!(
            "🤝 <b>Invita y gana</b>\n\n\
             Por cada amigo que se haga Premium te regalamos <b>{} días</b>.\n\n\
             Tu enlace:\nhttps://t.me/{}?start=ref_{}\n\n\
             Amigos Premium hasta ahora: {}",
            self.cfg.referral_bonus_days, bot, user.chat_id, user.referral_count
        )
    }

    fn cmd_terms(&self) -> String {
        format!(
            "📄 <b>TÉRMINOS</b>\n\n\
             • El servicio entrega señales informativas generadas automáticamente. No es asesoría financiera ni gestión de fondos.\n\
             • Tú decides si operas y asumes el riesgo. Puedes perder todo el capital, en especial con apalancamiento.\n\
             • Los resultados pasados no garantizan resultados futuros. /performance indica si incluye operaciones simuladas.\n\
             • Premium dura {} días por pago. Las suscripciones se cancelan desde los ajustes de Telegram.\n\
             • Reembolsos: escribe a {} dentro de 7 días del pago (/paysupport).",
            self.cfg.premium_days,
            esc(&self.cfg.support_contact)
        )
    }

    async fn cmd_status(&self) -> String {
        let risk = self.risk_manager.lock().await;
        let balance = risk.current_balance().to_f64().unwrap_or(0.0);
        let initial = risk.initial_capital().to_f64().unwrap_or(0.0);
        let pnl = risk.total_pnl().to_f64().unwrap_or(0.0);
        format!(
            "🤖 <b>ESTADO DEL BOT</b>\n\n\
             💰 Balance: ${:.2}\n\
             📈 P&amp;L: ${:.2} ({:.1}%)\n\
             🎯 Win rate: {:.1}% ({}G/{}P)\n\
             📉 Drawdown: {:.2}%\n\
             ⚠️ Riesgo: {:?}",
            balance,
            pnl,
            if initial > 0.0 { pnl / initial * 100.0 } else { 0.0 },
            risk.win_rate() * 100.0,
            risk.winning_trades(),
            risk.losing_trades(),
            risk.current_drawdown().to_f64().unwrap_or(0.0) * 100.0,
            risk.status()
        )
    }

    async fn cmd_balance(&self) -> String {
        let risk = self.risk_manager.lock().await;
        let balance = risk.current_balance().to_f64().unwrap_or(0.0);
        let initial = risk.initial_capital().to_f64().unwrap_or(0.0);
        let pnl = balance - initial;
        format!(
            "💰 <b>BALANCE</b>\n\nInicial: ${:.2}\nActual: ${:.2}\nP&amp;L: ${:.2} ({:.1}%)",
            initial,
            balance,
            pnl,
            if initial > 0.0 { pnl / initial * 100.0 } else { 0.0 }
        )
    }

    async fn cmd_trades(&self) -> String {
        let signals = self.get_signals(8).await;
        if signals.is_empty() {
            return "📊 No hay operaciones registradas.".to_string();
        }
        let mut msg = String::from("📊 <b>ÚLTIMAS SEÑALES</b>\n\n");
        for r in signals {
            let line = match &r.event {
                SignalEvent::Entry { symbol, price, .. } => format!("🟢 {} compra ${}", symbol, price.round_dp(6).normalize()),
                SignalEvent::StopMoved { symbol, stop, .. } => format!("🛡️ {} stop ${}", symbol, stop.round_dp(6).normalize()),
                SignalEvent::Exit { symbol, .. } => format!("🔴 {} cierre {:+.2}%", symbol, r.event.pnl_pct().unwrap_or(0.0)),
            };
            msg.push_str(&format!("{} — {}\n", r.at.format("%d-%m %H:%M"), line));
        }
        msg
    }

    async fn cmd_stats(&self) -> String {
        let s = self.get_stats().await;
        format!(
            "📊 <b>NEGOCIO</b>\n\n\
             Usuarios: {} (+{} en 7 días)\n\
             Premium activos: {} (pagando: {})\n\
             Bloquearon el bot: {}\n\n\
             Ingresos 30 días: <b>{} ⭐</b>\n\
             Ingresos totales: {} ⭐\n\
             Precio actual: {} ⭐",
            s.total_users,
            s.new_users_7d,
            s.premium_users,
            s.paying_users,
            s.blocked_users,
            s.revenue_stars_30d,
            s.revenue_stars_total,
            s.premium_price_stars
        )
    }

    async fn cmd_users(&self) -> String {
        let users = self.get_users().await;
        if users.is_empty() {
            return "📭 No hay usuarios.".to_string();
        }
        let mut msg = format!("👥 <b>USUARIOS ({})</b>\n\n", users.len());
        // Telegram limita los mensajes a 4096 caracteres
        for u in users.iter().rev().take(40) {
            msg.push_str(&format!(
                "{} <b>{}</b> @{} <code>{}</code>{}\n",
                if u.is_premium { "💎" } else if u.authorized { "🆓" } else { "⛔" },
                esc(&u.first_name),
                esc(&u.username),
                u.chat_id,
                if u.blocked { " 🚫" } else { "" }
            ));
        }
        if users.len() > 40 {
            msg.push_str("\n… (lista completa en el dashboard)");
        }
        msg
    }

    async fn cmd_refund(&self, user_id: &str, charge_id: &str) -> String {
        let payload = json!({ "user_id": user_id.parse::<i64>().unwrap_or(0), "telegram_payment_charge_id": charge_id });
        match self.api("refundStarPayment", &payload).await {
            Ok(_) => {
                let _ = self.revoke_premium(user_id).await;
                format!("✅ Reembolso hecho a {} y Premium revocado.", user_id)
            }
            Err((_, e)) => format!("❌ Reembolso falló: {}", e),
        }
    }

    // =============================================
    // PROCESAMIENTO
    // =============================================

    async fn process_message(&self, message: &serde_json::Value) -> Result<(), String> {
        let chat_id = message["chat"]["id"].as_i64().unwrap_or(0).to_string();
        let username = message["from"]["username"].as_str().unwrap_or("sin_username");
        let first_name = message["from"]["first_name"].as_str().unwrap_or("Usuario");
        let (user, is_new) = self.get_or_create_user(&chat_id, username, first_name).await;

        if let Some(payment) = message.get("successful_payment") {
            self.handle_successful_payment(&chat_id, payment).await;
            return Ok(());
        }

        let text = message["text"].as_str().unwrap_or("").trim().to_string();
        if !text.starts_with('/') {
            return Ok(());
        }
        info!("📨 @{} ({}): {}", username, chat_id, text);

        let (head, args) = text.split_once(' ').unwrap_or((text.as_str(), ""));
        // "/cmd@NombreBot" en grupos
        let cmd = head.split('@').next().unwrap_or(head);
        let args = args.trim();
        let admin = self.is_admin(&chat_id).await;

        let mut keyboard = None;
        let response = match cmd {
            "/start" => self.cmd_start(&user, is_new, args).await,
            "/help" => self.cmd_help(&user).await,
            "/premium" | "/comprar" => {
                let (t, k) = self.cmd_premium(&user).await;
                keyboard = k;
                t
            }
            "/trial" | "/prueba" => self.cmd_trial(&user).await,
            "/plan" | "/miplan" => self.cmd_plan(&user).await,
            "/signals" | "/senales" => self.cmd_signals(&user).await,
            "/performance" | "/resultados" => self.performance_text().await,
            "/referral" | "/referidos" => self.cmd_referral(&user).await,
            "/terms" | "/terminos" => self.cmd_terms(),
            "/paysupport" => format!(
                "💬 Para problemas con pagos o reembolsos escribe a {} indicando tu ID <code>{}</code>.",
                esc(&self.cfg.support_contact),
                chat_id
            ),
            "/subscribe" => {
                if !user.authorized {
                    "⛔ Necesitas autorización para suscribirte.".to_string()
                } else {
                    let _ = self.update_user(&chat_id, |u| u.subscribed = true).await;
                    "🔔 Avisos activados.".to_string()
                }
            }
            "/unsubscribe" => {
                let _ = self.update_user(&chat_id, |u| u.subscribed = false).await;
                "🔕 Avisos pausados. /subscribe para reactivarlos.".to_string()
            }
            // ---- Admin ----
            _ if !admin => "❓ Comando no reconocido. /help".to_string(),
            "/status" => self.cmd_status().await,
            "/balance" => self.cmd_balance().await,
            "/trades" => self.cmd_trades().await,
            "/stats" => self.cmd_stats().await,
            "/users" => self.cmd_users().await,
            "/authorize" => match self.authorize_user(args).await {
                Ok(_) => format!("✅ {} autorizado.", args),
                Err(e) => format!("❌ {}", e),
            },
            "/deauthorize" => match self.deauthorize_user(args).await {
                Ok(_) => format!("✅ {} desautorizado.", args),
                Err(e) => format!("❌ {}", e),
            },
            "/grant" => {
                let mut it = args.split_whitespace();
                match (it.next(), it.next().and_then(|d| d.parse::<i64>().ok())) {
                    (Some(id), Some(days)) => match self.grant_premium(id, days).await {
                        Ok(until) => format!("💎 {} Premium hasta {}", id, until.format("%d-%m-%Y")),
                        Err(e) => format!("❌ {}", e),
                    },
                    _ => "Uso: /grant &lt;id&gt; &lt;días&gt;".to_string(),
                }
            }
            "/revoke" => match self.revoke_premium(args).await {
                Ok(_) => format!("✅ Premium revocado a {}", args),
                Err(e) => format!("❌ {}", e),
            },
            "/refund" => {
                let mut it = args.split_whitespace();
                match (it.next(), it.next()) {
                    (Some(id), Some(charge)) => self.cmd_refund(id, charge).await,
                    _ => "Uso: /refund &lt;id&gt; &lt;charge_id&gt; (ver pagos en el dashboard)".to_string(),
                }
            }
            "/broadcast" | "/broadcast_free" | "/broadcast_premium" if !args.is_empty() => {
                let segment = cmd.trim_start_matches("/broadcast").trim_start_matches('_');
                let segment = if segment.is_empty() { "all" } else { segment };
                match self.broadcast_segment(args, segment).await {
                    Ok(n) => format!("✅ Enviado a {} usuarios ({}).", n, segment),
                    Err(e) => format!("❌ {}", e),
                }
            }
            _ => "❓ Comando no reconocido. /help".to_string(),
        };

        self.deliver(&chat_id, &response, keyboard).await
    }

    async fn get_updates(&self, offset: &mut i64) -> Result<Vec<serde_json::Value>, String> {
        let params = json!({
            "offset": offset,
            "timeout": 30,
            "allowed_updates": ["message", "pre_checkout_query"]
        });
        let result = self.api("getUpdates", &params).await.map_err(|(c, e)| format!("{} {}", c, e))?;
        let updates = result.as_array().cloned().unwrap_or_default();
        if let Some(id) = updates.last().and_then(|u| u["update_id"].as_i64()) {
            *offset = id + 1;
        }
        Ok(updates)
    }

    async fn init_bot_profile(&self) {
        match self.api("getMe", &json!({})).await {
            Ok(me) => {
                let name = me["username"].as_str().unwrap_or_default().to_string();
                info!("🤖 Bot de Telegram: @{}", name);
                *self.bot_username.lock().await = name;
            }
            Err((_, e)) => error!("❌ getMe falló: {}", e),
        }
        let commands = json!({ "commands": [
            { "command": "start", "description": "Inicio" },
            { "command": "premium", "description": "Suscribirse a Premium" },
            { "command": "trial", "description": "Prueba Premium gratis" },
            { "command": "performance", "description": "Resultados de las señales" },
            { "command": "signals", "description": "Señales abiertas (Premium)" },
            { "command": "plan", "description": "Mi plan" },
            { "command": "referral", "description": "Invita y gana días Premium" },
            { "command": "terms", "description": "Términos" },
            { "command": "paysupport", "description": "Ayuda con pagos" },
            { "command": "help", "description": "Ayuda" }
        ]});
        if let Err((_, e)) = self.api("setMyCommands", &commands).await {
            warn!("setMyCommands falló: {}", e);
        }
    }

    // =============================================
    // LOOP PRINCIPAL
    // =============================================

    pub async fn run(&self) {
        self.init_bot_profile().await;

        let me = self.clone();
        tokio::spawn(async move { me.signal_loop().await });
        let me = self.clone();
        tokio::spawn(async move { me.maintenance_loop().await });

        info!("🤖 Bot de Telegram iniciado. Esperando mensajes...");
        let mut offset = 0;
        loop {
            match self.get_updates(&mut offset).await {
                Ok(updates) => {
                    for update in updates {
                        if let Some(q) = update.get("pre_checkout_query") {
                            // Telegram exige respuesta en < 10 s
                            self.handle_pre_checkout(q).await;
                        } else if let Some(message) = update.get("message") {
                            if let Err(e) = self.process_message(message).await {
                                debug!("Mensaje no entregado: {}", e);
                            }
                        }
                    }
                }
                Err(e) => {
                    error!("Error obteniendo updates: {}", e);
                    time::sleep(Duration::from_secs(5)).await;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_user_json_still_loads() {
        let json = r#"[{"chat_id":"1","username":"a","first_name":"A","subscribed":true,
                        "authorized":true,"last_interaction":"2025-01-01T00:00:00Z"}]"#;
        let users: Vec<TelegramUser> = serde_json::from_str(json).unwrap();
        assert_eq!(users.len(), 1);
        assert!(!users[0].premium_active());
        assert!(users[0].receives_alerts());
    }

    #[test]
    fn extend_premium_stacks_on_active_plan() {
        let mut u = TelegramUser::new("1", "a", "A", true, false);
        let first = u.extend_premium(30);
        let second = u.extend_premium(7);
        assert!(u.premium_active());
        assert_eq!((second - first).num_days(), 7);
    }

    #[test]
    fn html_is_escaped() {
        assert_eq!(esc("<b>&"), "&lt;b&gt;&amp;");
    }
}
