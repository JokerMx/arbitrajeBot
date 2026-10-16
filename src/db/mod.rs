//! Módulo de persistencia con MariaDB

use sqlx::{MySql, Pool, MySqlPool};
use anyhow::Result;
use tracing::{info, error};

pub mod models;
pub mod config_repository;
pub mod store;

pub use config_repository::ConfigRepository;
pub use store::Store;

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
    
    /// URL sin la contraseña, para mostrar en logs
    pub fn redacted_url(&self) -> String {
        match (self.url.find("://"), self.url.rfind('@')) {
            (Some(scheme), Some(at)) if at > scheme => {
                let creds = &self.url[scheme + 3..at];
                let user = creds.split(':').next().unwrap_or("");
                format!("{}{}:***{}", &self.url[..scheme + 3], user, &self.url[at..])
            }
            _ => self.url.clone(),
        }
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
        
        info!("✅ Conectado a MariaDB en: {}", config.redacted_url());
        
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
        
        // Tabla de apalancamiento por símbolo
        let leverage_table = self.config.table_name("leverage");
        sqlx::query(&format!(r#"
            CREATE TABLE IF NOT EXISTS `{}` (
                id INT PRIMARY KEY AUTO_INCREMENT,
                exchange VARCHAR(50) NOT NULL,
                symbol VARCHAR(20) NOT NULL,
                leverage INT NOT NULL DEFAULT 1,
                max_leverage INT NOT NULL DEFAULT 10,
                updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
                UNIQUE KEY unique_exchange_symbol (exchange, symbol)
            ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci
        "#, leverage_table))
        .execute(&self.pool)
        .await?;
        
        // Tabla de protección de capital
        let protection_table = self.config.table_name("protection");
        sqlx::query(&format!(r#"
            CREATE TABLE IF NOT EXISTS `{}` (
                id INT PRIMARY KEY AUTO_INCREMENT,
                max_capital_at_risk_pct DECIMAL(5,4) NOT NULL DEFAULT 0.25,
                auto_stop_loss_enabled BOOLEAN NOT NULL DEFAULT TRUE,
                auto_stop_loss_pct DECIMAL(5,4) NOT NULL DEFAULT 0.15,
                max_leverage INT NOT NULL DEFAULT 10,
                emergency_withdraw_enabled BOOLEAN NOT NULL DEFAULT FALSE,
                updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP
            ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci
        "#, protection_table))
        .execute(&self.pool)
        .await?;
        
        // Insertar config de protección por defecto si no existe
        sqlx::query(&format!(r#"
            INSERT IGNORE INTO `{}` (id, max_capital_at_risk_pct, auto_stop_loss_enabled, auto_stop_loss_pct, max_leverage)
            VALUES (1, 0.25, TRUE, 0.15, 10)
        "#, protection_table))
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
#[cfg(test)]
mod tests {
    use super::DbConfig;

    fn cfg(url: &str) -> DbConfig {
        DbConfig { url: url.to_string(), table_prefix: "BOT_".to_string() }
    }

    #[test]
    fn password_is_hidden_in_logs() {
        assert_eq!(cfg("mysql://bot:s3cr3t@db:3306/x").redacted_url(), "mysql://bot:***@db:3306/x");
        // Contraseña con '@' (la URL usa el último '@' como separador del host)
        assert_eq!(cfg("mysql://bot:a@b@db/x").redacted_url(), "mysql://bot:***@db/x");
        assert_eq!(cfg("mysql://db/x").redacted_url(), "mysql://db/x");
    }
}
