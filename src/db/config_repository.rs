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

fn read_env_or_default<T: FromStr>(key: &str, default: T) -> T {
    std::env::var(key).unwrap_or_default().parse().unwrap_or(default)
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
    
    /// Obtener todos los mercados - prioridad .env > BD
    pub async fn get_markets(&self) -> Result<Vec<MarketConfigDB>> {
        let _table = format!("{}markets", self.table_prefix);
        
        let symbols = vec!["BTC", "ETH", "BNB", "SOL", "XRP"];
        let mut markets = Vec::new();
        
        // Leer cada símbolo desde .env y sincronizar con BD
        for symbol in &symbols {
            let enabled = std::env::var(format!("MARKETS_ENABLED_{}", symbol))
                .unwrap_or_else(|_| "false".to_string())
                .parse()
                .unwrap_or(false);
            
            let margin_alloc = Decimal::from_str(
                &std::env::var(format!("MARKETS_MARGIN_ALLOC_{}", symbol))
                    .unwrap_or_else(|_| "0.10".to_string())
            ).unwrap_or(Decimal::new(10, 2));
            
            let min_size = Decimal::from_str(
                &std::env::var(format!("MARKETS_MIN_SIZE_{}", symbol))
                    .unwrap_or_else(|_| "0.00001".to_string())
            ).unwrap_or(Decimal::new(1, 5));
            
            let price_decimals = match *symbol {
                "BTC" | "ETH" => 1,
                "BNB" | "SOL" => 2,
                _ => 2,
            };
            
            let size_decimals = match *symbol {
                "BTC" => 8,
                "ETH" => 6,
                "BNB" => 5,
                "SOL" => 4,
                "XRP" => 2,
                _ => 8,
            };
            
            // Sincronizar con BD
            let _ = self.update_market(symbol, Some(enabled), Some(margin_alloc), Some(min_size)).await;
            
            markets.push(MarketConfigDB {
                symbol: symbol.to_string(),
                enabled,
                margin_alloc,
                min_size,
                price_decimals,
                size_decimals,
            });
        }
        
        Ok(markets)
    }
    
    /// Crear mercados por defecto
    async fn create_default_markets(&self) -> Result<Vec<MarketConfigDB>> {
        let btc_enabled = std::env::var("MARKETS_ENABLED_BTC")
            .unwrap_or_else(|_| "true".to_string())
            .parse()
            .unwrap_or(true);
        let eth_enabled = std::env::var("MARKETS_ENABLED_ETH")
            .unwrap_or_else(|_| "false".to_string())
            .parse()
            .unwrap_or(false);
        let bnb_enabled = std::env::var("MARKETS_ENABLED_BNB")
            .unwrap_or_else(|_| "false".to_string())
            .parse()
            .unwrap_or(false);
        let sol_enabled = std::env::var("MARKETS_ENABLED_SOL")
            .unwrap_or_else(|_| "false".to_string())
            .parse()
            .unwrap_or(false);
        let xrp_enabled = std::env::var("MARKETS_ENABLED_XRP")
            .unwrap_or_else(|_| "false".to_string())
            .parse()
            .unwrap_or(false);

        let default_markets = vec![
            MarketConfigDB { 
                symbol: "BTC".to_string(), 
                enabled: btc_enabled, 
                margin_alloc: Decimal::from_str(&std::env::var("MARKETS_MARGIN_ALLOC_BTC").unwrap_or_else(|_| "0.10".to_string())).unwrap_or(Decimal::new(10, 2)),
                min_size: Decimal::from_str(&std::env::var("MARKETS_MIN_SIZE_BTC").unwrap_or_else(|_| "0.00001".to_string())).unwrap_or(Decimal::new(1, 5)),
                price_decimals: 1, 
                size_decimals: 8 
            },
            MarketConfigDB { 
                symbol: "ETH".to_string(), 
                enabled: eth_enabled, 
                margin_alloc: Decimal::from_str(&std::env::var("MARKETS_MARGIN_ALLOC_ETH").unwrap_or_else(|_| "0.10".to_string())).unwrap_or(Decimal::new(10, 2)),
                min_size: Decimal::from_str(&std::env::var("MARKETS_MIN_SIZE_ETH").unwrap_or_else(|_| "0.0001".to_string())).unwrap_or(Decimal::new(1, 4)),
                price_decimals: 1, 
                size_decimals: 6 
            },
            MarketConfigDB { 
                symbol: "BNB".to_string(), 
                enabled: bnb_enabled, 
                margin_alloc: Decimal::from_str(&std::env::var("MARKETS_MARGIN_ALLOC_BNB").unwrap_or_else(|_| "0.10".to_string())).unwrap_or(Decimal::new(10, 2)),
                min_size: Decimal::from_str(&std::env::var("MARKETS_MIN_SIZE_BNB").unwrap_or_else(|_| "0.001".to_string())).unwrap_or(Decimal::new(1, 3)),
                price_decimals: 2, 
                size_decimals: 5 
            },
            MarketConfigDB { 
                symbol: "SOL".to_string(), 
                enabled: sol_enabled, 
                margin_alloc: Decimal::from_str(&std::env::var("MARKETS_MARGIN_ALLOC_SOL").unwrap_or_else(|_| "0.10".to_string())).unwrap_or(Decimal::new(10, 2)),
                min_size: Decimal::from_str(&std::env::var("MARKETS_MIN_SIZE_SOL").unwrap_or_else(|_| "0.01".to_string())).unwrap_or(Decimal::new(1, 2)),
                price_decimals: 2, 
                size_decimals: 4 
            },
            MarketConfigDB { 
                symbol: "XRP".to_string(), 
                enabled: xrp_enabled, 
                margin_alloc: Decimal::from_str(&std::env::var("MARKETS_MARGIN_ALLOC_XRP").unwrap_or_else(|_| "0.10".to_string())).unwrap_or(Decimal::new(10, 2)),
                min_size: Decimal::from_str(&std::env::var("MARKETS_MIN_SIZE_XRP").unwrap_or_else(|_| "0.5".to_string())).unwrap_or(Decimal::new(5, 1)),
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
        
        // Verificar si existe
        let exists: Vec<_> = sqlx::query(&format!(
            r#"SELECT symbol FROM `{}` WHERE symbol = ?"#,
            table
        ))
        .bind(symbol)
        .fetch_all(&self.pool)
        .await?;

        if exists.is_empty() {
            // Insertar si no existe
            let margin_str = margin_alloc.unwrap_or(Decimal::new(10, 2)).to_string();
            let min_str = min_size.unwrap_or(Decimal::new(1, 5)).to_string();
            sqlx::query(&format!(
                r#"INSERT INTO `{}` (symbol, enabled, margin_alloc, min_size, price_decimals, size_decimals) VALUES (?, ?, ?, ?, ?, ?)"#,
                table
            ))
            .bind(symbol)
            .bind(enabled.unwrap_or(false))
            .bind(margin_str)
            .bind(min_str)
            .bind(1)
            .bind(8)
            .execute(&self.pool)
            .await?;
            info!("✅ Mercado {} creado", symbol);
            return Ok(());
        }
        
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
        let protection = self.get_protection_config().await?;
        
        let leverage = self.get_default_leverage_configs().await?;
        
        Ok(FullConfigDB {
            initial_capital,
            strategy,
            risk,
            markets,
            telegram,
            leverage,
            protection,
            updated_at: None,
        })
    }
    
    async fn get_default_leverage_configs(&self) -> Result<Vec<LeverageConfigDB>> {
        let symbols = vec!["BTC", "ETH", "BNB", "SOL", "XRP"];
        let exchanges = vec!["simulated", "hyperliquid_mainnet", "bybit"];
        
        let mut configs = Vec::new();
        for exchange in exchanges {
            for symbol in &symbols {
                let lev = self.get_leverage(exchange, symbol).await?;
                configs.push(lev);
            }
        }
        
        Ok(configs)
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
        
        for lev in &config.leverage {
            self.update_leverage(&lev.exchange, &lev.symbol, lev.leverage, lev.max_leverage).await?;
        }
        
        self.update_protection_config(&config.protection).await?;
        
        info!("✅ Configuración completa guardada en BD");
        Ok(())
    }
    
    /// Obtener apalancamiento por símbolo
    pub async fn get_leverage(&self, exchange: &str, symbol: &str) -> Result<LeverageConfigDB> {
        let table = format!("{}leverage", self.table_prefix);
        
        let env_key = format!("LEVERAGE_{}_{}", exchange.to_uppercase(), symbol.to_uppercase());
        let env_value = std::env::var(&env_key);
        
        let row = sqlx::query(&format!(
            r#"SELECT CAST(leverage AS CHAR), CAST(max_leverage AS CHAR) FROM `{}` WHERE exchange = ? AND symbol = ?"#,
            table
        ))
        .bind(exchange)
        .bind(symbol)
        .fetch_optional(&self.pool)
        .await?;
        
        // Prioridad: .env > BD
        if let Ok(env_leverage) = env_value {
            let leverage: i32 = env_leverage.parse().unwrap_or(1);
            let max_leverage = read_env_or_default("MAX_ALLOWABLE_LEVERAGE", 10);
            info!("📈 Leverage desde .env: {}={} para {}/{}", env_key, leverage, exchange, symbol);
            
            // Guardar en BD si es diferente
            if let Some(row) = row {
                let db_leverage: String = row.get(0);
                if db_leverage.parse::<i32>().unwrap_or(1) != leverage {
                    let _ = self.update_leverage(exchange, symbol, leverage, max_leverage).await;
                }
            }
            
            return Ok(LeverageConfigDB {
                exchange: exchange.to_string(),
                symbol: symbol.to_string(),
                leverage,
                max_leverage,
            });
        }
        
        if let Some(row) = row {
            let leverage_str: String = row.get(0);
            let max_leverage_str: String = row.get(1);
            
            Ok(LeverageConfigDB {
                exchange: exchange.to_string(),
                symbol: symbol.to_string(),
                leverage: leverage_str.parse().unwrap_or(1),
                max_leverage: max_leverage_str.parse().unwrap_or(10),
            })
        } else {
            let leverage = 1;
            let max_leverage = read_env_or_default("MAX_ALLOWABLE_LEVERAGE", 10);
            
            Ok(LeverageConfigDB {
                exchange: exchange.to_string(),
                symbol: symbol.to_string(),
                leverage,
                max_leverage,
            })
        }
    }
    
    /// Actualizar apalancamiento
    pub async fn update_leverage(&self, exchange: &str, symbol: &str, leverage: i32, max_leverage: i32) -> Result<()> {
        let table = format!("{}leverage", self.table_prefix);
        let max_leverage = max_leverage.max(1).min(100);
        let leverage = leverage.max(1).min(max_leverage);
        
        sqlx::query(&format!(
            r#"INSERT INTO `{}` (exchange, symbol, leverage, max_leverage) VALUES (?, ?, ?, ?)
               ON DUPLICATE KEY UPDATE leverage = ?, max_leverage = ?"#,
            table
        ))
        .bind(exchange)
        .bind(symbol)
        .bind(leverage)
        .bind(max_leverage)
        .bind(leverage)
        .bind(max_leverage)
        .execute(&self.pool)
        .await?;
        
        info!("✅ Apalancamiento {} {}: {}x", exchange, symbol, leverage);
        Ok(())
    }
    
    /// Obtener configuración de protección
    pub async fn get_protection_config(&self) -> Result<ProtectionConfigDB> {
        let table = format!("{}protection", self.table_prefix);
        
        let row = sqlx::query(&format!(
            r#"SELECT CAST(max_capital_at_risk_pct AS CHAR), auto_stop_loss_enabled, CAST(auto_stop_loss_pct AS CHAR), max_leverage, emergency_withdraw_enabled FROM `{}` WHERE id = 1"#,
            table
        ))
        .fetch_optional(&self.pool)
        .await?;
        
        if let Some(row) = row {
            let max_capital_str: String = row.get(0);
            let stop_loss_str: String = row.get(2);
            
            Ok(ProtectionConfigDB {
                max_capital_at_risk_pct: Decimal::from_str(&max_capital_str).unwrap_or(Decimal::new(25, 2)),
                auto_stop_loss_enabled: row.get(1),
                auto_stop_loss_pct: Decimal::from_str(&stop_loss_str).unwrap_or(Decimal::new(15, 2)),
                max_leverage: row.get(3),
                emergency_withdraw_enabled: row.get(4),
            })
        } else {
            Ok(ProtectionConfigDB::default())
        }
    }
    
    /// Actualizar configuración de protección
    pub async fn update_protection_config(&self, config: &ProtectionConfigDB) -> Result<()> {
        let table = format!("{}protection", self.table_prefix);
        
        sqlx::query(&format!(
            r#"UPDATE `{}` SET 
                max_capital_at_risk_pct = ?,
                auto_stop_loss_enabled = ?,
                auto_stop_loss_pct = ?,
                max_leverage = ?,
                emergency_withdraw_enabled = ?
               WHERE id = 1"#,
            table
        ))
        .bind(config.max_capital_at_risk_pct.to_string())
        .bind(config.auto_stop_loss_enabled)
        .bind(config.auto_stop_loss_pct.to_string())
        .bind(config.max_leverage)
        .bind(config.emergency_withdraw_enabled)
        .execute(&self.pool)
        .await?;
        
        info!("✅ Configuración de protección actualizada");
        Ok(())
    }
}