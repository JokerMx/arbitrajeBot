//! Módulo de persistencia con MariaDB

use sqlx::{MySql, Pool, MySqlPool};
use anyhow::Result;
use tracing::{info, error};

pub mod models;
pub mod config_repository;

pub use config_repository::ConfigRepository;

/// Configuración de la base de datos
#[derive(Debug, Clone)]
pub struct DbConfig {
    pub url: String,
    pub table_prefix: String,
}

impl DbConfig {
    pub fn from_env() -> Result<Self> {
        let url = std::env::var("DATABASE_URL")
            .expect("DATABASE_URL must be set in .env");
        let table_prefix = std::env::var("DB_TABLE_PREFIX")
            .unwrap_or_else(|_| "BOT_".to_string());
        
        Ok(Self { url, table_prefix })
    }
    
    pub fn table_name(&self, name: &str) -> String {
        format!("{}{}", self.table_prefix, name)
    }
}

/// Pool de conexiones a MariaDB
pub struct Database {
    pub pool: Pool<MySql>,
    pub config: DbConfig,
}

impl Database {
    pub async fn new() -> Result<Self> {
        let config = DbConfig::from_env()?;
        let pool = MySqlPool::connect(&config.url).await?;
        
        info!("✅ Conectado a MariaDB en: {}", config.url);
        
        Ok(Self { pool, config })
    }
    
    /// Inicializar tablas
    pub async fn init_tables(&self) -> Result<()> {
        let config_table = self.config.table_name("config");
        let markets_table = self.config.table_name("markets");
        let telegram_table = self.config.table_name("telegram");
        
        // Tabla de configuración general
        sqlx::query(&format!(r#"
            CREATE TABLE IF NOT EXISTS `{}` (
                id INT PRIMARY KEY AUTO_INCREMENT,
                config_key VARCHAR(100) NOT NULL UNIQUE,
                config_value TEXT NOT NULL,
                config_type VARCHAR(50) NOT NULL DEFAULT 'string',
                updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                INDEX idx_key (config_key)
            ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci
        "#, config_table))
        .execute(&self.pool)
        .await?;
        
        // Tabla de mercados
        sqlx::query(&format!(r#"
            CREATE TABLE IF NOT EXISTS `{}` (
                id INT PRIMARY KEY AUTO_INCREMENT,
                symbol VARCHAR(20) NOT NULL UNIQUE,
                enabled BOOLEAN NOT NULL DEFAULT TRUE,
                margin_alloc DECIMAL(20,8) NOT NULL DEFAULT 0.12,
                min_size DECIMAL(20,8) NOT NULL DEFAULT 0.00001,
                price_decimals INT NOT NULL DEFAULT 2,
                size_decimals INT NOT NULL DEFAULT 8,
                updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                INDEX idx_symbol (symbol),
                INDEX idx_enabled (enabled)
            ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci
        "#, markets_table))
        .execute(&self.pool)
        .await?;
        
        // Tabla de configuración de Telegram
        sqlx::query(&format!(r#"
            CREATE TABLE IF NOT EXISTS `{}` (
                id INT PRIMARY KEY AUTO_INCREMENT,
                enabled BOOLEAN NOT NULL DEFAULT FALSE,
                bot_token VARCHAR(255) DEFAULT NULL,
                chat_id VARCHAR(100) DEFAULT NULL,
                notify_orders BOOLEAN NOT NULL DEFAULT TRUE,
                notify_fills BOOLEAN NOT NULL DEFAULT TRUE,
                notify_status BOOLEAN NOT NULL DEFAULT TRUE,
                updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci
        "#, telegram_table))
        .execute(&self.pool)
        .await?;
        
        info!("✅ Tablas inicializadas en MariaDB");
        
        Ok(())
    }
    
    /// Verificar conexión
    pub async fn health_check(&self) -> bool {
        match sqlx::query("SELECT 1").fetch_one(&self.pool).await {
            Ok(_) => true,
            Err(e) => {
                error!("❌ Health check falló: {}", e);
                false
            }
        }
    }
}