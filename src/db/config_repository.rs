//! Repositorio para configuración persistente

use sqlx::{MySqlPool, Row};
use anyhow::Result;
use tracing::{info, debug};
use rust_decimal::Decimal;
use std::str::FromStr;

use super::{Database, models::*};

pub struct ConfigRepository {
    pool: MySqlPool,
    table_prefix: String,
}

impl ConfigRepository {
    pub fn new(db: &Database) -> Self {
        Self {
            pool: db.pool.clone(),
            table_prefix: db.config.table_prefix.clone(),
        }
    }
    
    /// Obtener o crear configuración de estrategia
    pub async fn get_or_create_strategy_config(&self) -> Result<StrategyConfigDB> {
        let table = format!("{}config", self.table_prefix);
        
        let row = sqlx::query(&format!(
            r#"SELECT config_value FROM `{}` WHERE config_key = 'strategy_config'"#,
            table
        ))
        .fetch_optional(&self.pool)
        .await?;
        
        if let Some(row) = row {
            let value: String = row.get(0);
            let config: StrategyConfigDB = serde_json::from_str(&value)?;
            debug!("📦 Estrategia cargada desde BD");
            Ok(config)
        } else {
            let config = StrategyConfigDB::default();
            let json = serde_json::to_string(&config)?;
            
            sqlx::query(&format!(
                r#"INSERT INTO `{}` (config_key, config_value, config_type) VALUES ('strategy_config', ?, 'json')"#,
                table
            ))
            .bind(&json)
            .execute(&self.pool)
            .await?;
            
            info!("📦 Configuración de estrategia creada en BD");
            Ok(config)
        }
    }
    
    /// Actualizar configuración de estrategia
    pub async fn update_strategy_config(&self, config: &StrategyConfigDB) -> Result<()> {
        let table = format!("{}config", self.table_prefix);
        let json = serde_json::to_string(config)?;
        
        sqlx::query(&format!(
            r#"UPDATE `{}` SET config_value = ? WHERE config_key = 'strategy_config'"#,
            table
        ))
        .bind(&json)
        .execute(&self.pool)
        .await?;
        
        info!("✅ Estrategia actualizada en BD");
        Ok(())
    }
    
    /// Obtener o crear configuración de riesgo
    pub async fn get_or_create_risk_config(&self) -> Result<RiskConfigDB> {
        let table = format!("{}config", self.table_prefix);
        
        let row = sqlx::query(&format!(
            r#"SELECT config_value FROM `{}` WHERE config_key = 'risk_config'"#,
            table
        ))
        .fetch_optional(&self.pool)
        .await?;
        
        if let Some(row) = row {
            let value: String = row.get(0);
            let config: RiskConfigDB = serde_json::from_str(&value)?;
            Ok(config)
        } else {
            let config = RiskConfigDB::default();
            let json = serde_json::to_string(&config)?;
            
            sqlx::query(&format!(
                r#"INSERT INTO `{}` (config_key, config_value, config_type) VALUES ('risk_config', ?, 'json')"#,
                table
            ))
            .bind(&json)
            .execute(&self.pool)
            .await?;
            
            Ok(config)
        }
    }
    
    /// Actualizar configuración de riesgo
    pub async fn update_risk_config(&self, config: &RiskConfigDB) -> Result<()> {
        let table = format!("{}config", self.table_prefix);
        let json = serde_json::to_string(config)?;
        
        sqlx::query(&format!(
            r#"UPDATE `{}` SET config_value = ? WHERE config_key = 'risk_config'"#,
            table
        ))
        .bind(&json)
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
    
    /// Obtener o crear configuración de Telegram
    
    pub async fn get_or_create_telegram_config(&self) -> Result<TelegramConfigDB> {
    let table = format!("{}telegram", self.table_prefix);
    
    let row = sqlx::query(&format!(
        r#"SELECT enabled, bot_token, chat_id, notify_orders, notify_fills, notify_status 
           FROM `{}` LIMIT 1"#,
        table
    ))
    .fetch_optional(&self.pool)
    .await?;
    
    if let Some(row) = row {
        // Ya existe configuración, devolverla
        Ok(TelegramConfigDB {
            enabled: row.get(0),
            bot_token: row.get(1),
            chat_id: row.get(2),
            notify_orders: row.get(3),
            notify_fills: row.get(4),
            notify_status: row.get(5),
        })
    } else {
        // No existe configuración, crearla desde .env
        info!("📱 No hay configuración de Telegram en BD, cargando desde .env...");
        
        // Leer del archivo .env
        let bot_token = std::env::var("TELEGRAM_BOT_TOKEN")
            .unwrap_or_else(|_| "".to_string());
        let chat_id = std::env::var("TELEGRAM_CHAT_ID")
            .unwrap_or_else(|_| "".to_string());
        let enabled = !bot_token.is_empty() && !chat_id.is_empty();
        
        let config = TelegramConfigDB {
            enabled,
            bot_token: if enabled { Some(bot_token) } else { None },
            chat_id: if enabled { Some(chat_id) } else { None },
            notify_orders: std::env::var("TELEGRAM_NOTIFY_ORDERS")
                .unwrap_or_else(|_| "true".to_string())
                .parse()
                .unwrap_or(true),
            notify_fills: std::env::var("TELEGRAM_NOTIFY_FILLS")
                .unwrap_or_else(|_| "true".to_string())
                .parse()
                .unwrap_or(true),
            notify_status: std::env::var("TELEGRAM_NOTIFY_STATUS")
                .unwrap_or_else(|_| "true".to_string())
                .parse()
                .unwrap_or(true),
        };
        
        // Insertar en la base de datos
        sqlx::query(&format!(
            r#"INSERT INTO `{}` (enabled, bot_token, chat_id, notify_orders, notify_fills, notify_status)
               VALUES (?, ?, ?, ?, ?, ?)"#,
            table
        ))
        .bind(config.enabled)
        .bind(&config.bot_token)
        .bind(&config.chat_id)
        .bind(config.notify_orders)
        .bind(config.notify_fills)
        .bind(config.notify_status)
        .execute(&self.pool)
        .await?;
        
        if config.enabled {
            info!("✅ Configuración de Telegram guardada en BD con token: {}...", 
                  &config.bot_token.as_ref().unwrap().chars().take(10).collect::<String>());
        } else {
            info!("⚠️ Telegram no configurado en .env");
        }
        
        Ok(config)
    }
}    
    /// Actualizar configuración de Telegram
    pub async fn update_telegram_config(&self, config: &TelegramConfigDB) -> Result<()> {
        let table = format!("{}telegram", self.table_prefix);
        
        sqlx::query(&format!(
            r#"UPDATE `{}` SET 
                enabled = ?,
                bot_token = ?,
                chat_id = ?,
                notify_orders = ?,
                notify_fills = ?,
                notify_status = ?
               WHERE id = 1"#,
            table
        ))
        .bind(config.enabled)
        .bind(&config.bot_token)
        .bind(&config.chat_id)
        .bind(config.notify_orders)
        .bind(config.notify_fills)
        .bind(config.notify_status)
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
    
    /// Obtener capital inicial
    pub async fn get_or_create_initial_capital(&self) -> Result<Decimal> {
        let table = format!("{}config", self.table_prefix);
        
        let row = sqlx::query(&format!(
            r#"SELECT config_value FROM `{}` WHERE config_key = 'initial_capital'"#,
            table
        ))
        .fetch_optional(&self.pool)
        .await?;
        
        if let Some(row) = row {
            let value: String = row.get(0);
            let capital = Decimal::from_str(&value)?;
            Ok(capital)
        } else {
            let capital = std::env::var("INITIAL_CAPITAL")
                .unwrap_or_else(|_| "10.00".to_string())
                .parse::<Decimal>()
                .unwrap_or(Decimal::new(1000, 2));
            
            let value_str = capital.to_string();
            
            sqlx::query(&format!(
                r#"INSERT INTO `{}` (config_key, config_value, config_type) VALUES ('initial_capital', ?, 'decimal')"#,
                table
            ))
            .bind(&value_str)
            .execute(&self.pool)
            .await?;
            
            Ok(capital)
        }
    }
    
    /// Actualizar capital inicial
    pub async fn update_initial_capital(&self, capital: Decimal) -> Result<()> {
        let table = format!("{}config", self.table_prefix);
        let value = capital.to_string();
        
        sqlx::query(&format!(
            r#"UPDATE `{}` SET config_value = ? WHERE config_key = 'initial_capital'"#,
            table
        ))
        .bind(&value)
        .execute(&self.pool)
        .await?;
        
        info!("💰 Capital inicial actualizado a ${:.2}", capital);
        Ok(())
    }
    
    /// Obtener todos los mercados - usando String para leer decimales
    pub async fn get_markets(&self) -> Result<Vec<MarketConfigDB>> {
        let table = format!("{}markets", self.table_prefix);
        
        let rows = sqlx::query(&format!(
            r#"SELECT symbol, enabled, CAST(margin_alloc AS CHAR), CAST(min_size AS CHAR), price_decimals, size_decimals 
               FROM `{}` ORDER BY symbol"#,
            table
        ))
        .fetch_all(&self.pool)
        .await?;
        
        let mut markets = Vec::new();
        
        for row in rows {
            let margin_alloc_str: String = row.get(2);
            let min_size_str: String = row.get(3);
            
            markets.push(MarketConfigDB {
                symbol: row.get(0),
                enabled: row.get(1),
                margin_alloc: Decimal::from_str(&margin_alloc_str)?,
                min_size: Decimal::from_str(&min_size_str)?,
                price_decimals: row.get(4),
                size_decimals: row.get(5),
            });
        }
        
        if markets.is_empty() {
            markets = self.create_default_markets().await?;
        }
        
        Ok(markets)
    }
    
    /// Crear mercados por defecto
    async fn create_default_markets(&self) -> Result<Vec<MarketConfigDB>> {
        let default_markets = vec![
            MarketConfigDB { 
                symbol: "BTC".to_string(), 
                enabled: true, 
                margin_alloc: Decimal::new(12, 2),      // 0.12
                min_size: Decimal::new(1, 5),           // 0.00001
                price_decimals: 1, 
                size_decimals: 8 
            },
            MarketConfigDB { 
                symbol: "ETH".to_string(), 
                enabled: true, 
                margin_alloc: Decimal::new(12, 2),      // 0.12
                min_size: Decimal::new(1, 4),           // 0.0001
                price_decimals: 1, 
                size_decimals: 6 
            },
            MarketConfigDB { 
                symbol: "BNB".to_string(), 
                enabled: true, 
                margin_alloc: Decimal::new(12, 2),      // 0.12
                min_size: Decimal::new(1, 3),           // 0.001
                price_decimals: 2, 
                size_decimals: 5 
            },
            MarketConfigDB { 
                symbol: "SOL".to_string(), 
                enabled: true, 
                margin_alloc: Decimal::new(12, 2),      // 0.12
                min_size: Decimal::new(1, 2),           // 0.01
                price_decimals: 2, 
                size_decimals: 4 
            },
            MarketConfigDB { 
                symbol: "XRP".to_string(), 
                enabled: true, 
                margin_alloc: Decimal::new(12, 2),      // 0.12
                min_size: Decimal::new(5, 1),           // 0.5
                price_decimals: 4, 
                size_decimals: 2 
            },
        ];
        
        let table = format!("{}markets", self.table_prefix);
        
        for market in &default_markets {
            sqlx::query(&format!(
                r#"INSERT INTO `{}` (symbol, enabled, margin_alloc, min_size, price_decimals, size_decimals)
                   VALUES (?, ?, ?, ?, ?, ?)"#,
                table
            ))
            .bind(&market.symbol)
            .bind(market.enabled)
            .bind(market.margin_alloc.to_string())
            .bind(market.min_size.to_string())
            .bind(market.price_decimals)
            .bind(market.size_decimals)
            .execute(&self.pool)
            .await?;
        }
        
        info!("📦 Mercados por defecto creados en BD");
        Ok(default_markets)
    }
    
    /// Actualizar un mercado
    pub async fn update_market(&self, symbol: &str, enabled: Option<bool>, margin_alloc: Option<Decimal>, min_size: Option<Decimal>) -> Result<()> {
        let table = format!("{}markets", self.table_prefix);
        
        if let Some(enabled_val) = enabled {
            sqlx::query(&format!(
                r#"UPDATE `{}` SET enabled = ? WHERE symbol = ?"#,
                table
            ))
            .bind(enabled_val)
            .bind(symbol)
            .execute(&self.pool)
            .await?;
        }
        
        if let Some(margin_alloc_val) = margin_alloc {
            sqlx::query(&format!(
                r#"UPDATE `{}` SET margin_alloc = ? WHERE symbol = ?"#,
                table
            ))
            .bind(margin_alloc_val.to_string())
            .bind(symbol)
            .execute(&self.pool)
            .await?;
        }
        
        if let Some(min_size_val) = min_size {
            sqlx::query(&format!(
                r#"UPDATE `{}` SET min_size = ? WHERE symbol = ?"#,
                table
            ))
            .bind(min_size_val.to_string())
            .bind(symbol)
            .execute(&self.pool)
            .await?;
        }
        
        info!("✅ Mercado {} actualizado", symbol);
        Ok(())
    }
    
    /// Obtener configuración completa
    pub async fn get_full_config(&self) -> Result<FullConfigDB> {
        let strategy = self.get_or_create_strategy_config().await?;
        let risk = self.get_or_create_risk_config().await?;
        let markets = self.get_markets().await?;
        let telegram = self.get_or_create_telegram_config().await?;
        let initial_capital = self.get_or_create_initial_capital().await?;
        
        Ok(FullConfigDB {
            initial_capital,
            strategy,
            risk,
            markets,
            telegram,
            updated_at: None,
        })
    }
    
    /// Guardar configuración completa
    pub async fn save_full_config(&self, config: &FullConfigDB) -> Result<()> {
        self.update_strategy_config(&config.strategy).await?;
        self.update_risk_config(&config.risk).await?;
        self.update_telegram_config(&config.telegram).await?;
        self.update_initial_capital(config.initial_capital).await?;
        
        for market in &config.markets {
            self.update_market(
                &market.symbol,
                Some(market.enabled),
                Some(market.margin_alloc),
                Some(market.min_size),
            ).await?;
        }
        
        info!("✅ Configuración completa guardada en BD");
        Ok(())
    }
}