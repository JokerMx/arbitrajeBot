//! Persistencia del estado operativo en MariaDB (antes en archivos JSON):
//! historial de señales, fotos de equity, usuarios y pagos de Telegram y
//! posiciones abiertas de las estrategias. Así nada se pierde al detener o
//! reiniciar el bot.

use anyhow::Result;
use chrono::{DateTime, Utc};
use sqlx::{MySqlPool, Row};
use std::path::Path;
use tracing::{info, warn};

use crate::signals::{EquityPoint, SignalRecord};

#[derive(Clone)]
pub struct Store {
    pool: MySqlPool,
    prefix: String,
}

impl Store {
    pub fn new(pool: MySqlPool, prefix: &str) -> Self {
        Self { pool, prefix: prefix.to_string() }
    }

    fn t(&self, name: &str) -> String {
        format!("`{}{}`", self.prefix, name)
    }

    pub async fn init_tables(&self) -> Result<()> {
        let stmts = [
            format!(
                "CREATE TABLE IF NOT EXISTS {} (
                    id BIGINT UNSIGNED PRIMARY KEY,
                    at DATETIME(3) NOT NULL,
                    kind VARCHAR(16) NOT NULL,
                    symbol VARCHAR(32) NOT NULL,
                    strategy VARCHAR(16) NOT NULL,
                    paper BOOLEAN NOT NULL,
                    free_released BOOLEAN NOT NULL DEFAULT FALSE,
                    event LONGTEXT NOT NULL,
                    INDEX idx_at (at), INDEX idx_symbol (symbol)
                ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci",
                self.t("signals")
            ),
            format!(
                "CREATE TABLE IF NOT EXISTS {} (
                    t DATETIME(3) PRIMARY KEY,
                    equity DOUBLE NOT NULL
                ) ENGINE=InnoDB",
                self.t("equity_snapshots")
            ),
            format!(
                "CREATE TABLE IF NOT EXISTS {} (
                    chat_id VARCHAR(32) PRIMARY KEY,
                    data LONGTEXT NOT NULL,
                    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP
                ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci",
                self.t("tg_users")
            ),
            format!(
                "CREATE TABLE IF NOT EXISTS {} (
                    id BIGINT UNSIGNED PRIMARY KEY AUTO_INCREMENT,
                    chat_id VARCHAR(32) NOT NULL,
                    stars BIGINT NOT NULL,
                    charge_id VARCHAR(191) NOT NULL,
                    at DATETIME(3) NOT NULL,
                    data LONGTEXT NOT NULL,
                    UNIQUE KEY uq_charge (charge_id), INDEX idx_chat (chat_id)
                ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci",
                self.t("tg_payments")
            ),
            format!(
                "CREATE TABLE IF NOT EXISTS {} (
                    strategy VARCHAR(32) NOT NULL,
                    symbol VARCHAR(32) NOT NULL,
                    data LONGTEXT NOT NULL,
                    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
                    PRIMARY KEY (strategy, symbol)
                ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci",
                self.t("open_positions")
            ),
        ];
        for s in stmts {
            sqlx::query(&s).execute(&self.pool).await?;
        }
        Ok(())
    }

    // ---------------- Señales ----------------

    pub async fn load_signals(&self) -> Result<Vec<SignalRecord>> {
        let rows = sqlx::query(&format!(
            "SELECT id, at, free_released, event FROM {} ORDER BY id",
            self.t("signals")
        ))
        .fetch_all(&self.pool)
        .await?;
        let mut out = Vec::with_capacity(rows.len());
        for r in rows {
            let event: String = r.get("event");
            match serde_json::from_str(&event) {
                Ok(ev) => out.push(SignalRecord {
                    id: r.get::<u64, _>("id"),
                    at: r.get::<DateTime<Utc>, _>("at"),
                    free_released: r.get::<bool, _>("free_released"),
                    event: ev,
                }),
                Err(e) => warn!("Señal ilegible en la BD: {}", e),
            }
        }
        Ok(out)
    }

    pub async fn insert_signal(&self, rec: &SignalRecord) -> Result<()> {
        sqlx::query(&format!(
            "INSERT INTO {} (id, at, kind, symbol, strategy, paper, free_released, event)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)
             ON DUPLICATE KEY UPDATE free_released = VALUES(free_released), event = VALUES(event)",
            self.t("signals")
        ))
        .bind(rec.id)
        .bind(rec.at)
        .bind(rec.event.kind_name())
        .bind(rec.event.symbol())
        .bind(rec.event.strategy())
        .bind(rec.event.is_paper())
        .bind(rec.free_released)
        .bind(serde_json::to_string(&rec.event)?)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn mark_released(&self, ids: &[u64]) -> Result<()> {
        if ids.is_empty() {
            return Ok(());
        }
        let list = ids.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(",");
        sqlx::query(&format!(
            "UPDATE {} SET free_released = TRUE WHERE id IN ({})",
            self.t("signals"),
            list
        ))
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    // ---------------- Equity ----------------

    pub async fn load_equity(&self) -> Result<Vec<EquityPoint>> {
        let rows = sqlx::query(&format!("SELECT t, equity FROM {} ORDER BY t", self.t("equity_snapshots")))
            .fetch_all(&self.pool)
            .await?;
        Ok(rows
            .into_iter()
            .map(|r| EquityPoint { t: r.get::<DateTime<Utc>, _>("t"), equity: r.get::<f64, _>("equity") })
            .collect())
    }

    pub async fn insert_equity(&self, p: &EquityPoint) -> Result<()> {
        sqlx::query(&format!(
            "INSERT INTO {} (t, equity) VALUES (?, ?) ON DUPLICATE KEY UPDATE equity = VALUES(equity)",
            self.t("equity_snapshots")
        ))
        .bind(p.t)
        .bind(p.equity)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn delete_equity(&self, ts: &[DateTime<Utc>]) -> Result<()> {
        for chunk in ts.chunks(500) {
            let marks = vec!["?"; chunk.len()].join(",");
            let sql = format!("DELETE FROM {} WHERE t IN ({})", self.t("equity_snapshots"), marks);
            let mut q = sqlx::query(&sql);
            for t in chunk {
                q = q.bind(*t);
            }
            q.execute(&self.pool).await?;
        }
        Ok(())
    }

    // ---------------- Telegram ----------------

    pub async fn load_tg_users_json(&self) -> Result<Vec<String>> {
        let rows = sqlx::query(&format!("SELECT data FROM {}", self.t("tg_users")))
            .fetch_all(&self.pool)
            .await?;
        Ok(rows.into_iter().map(|r| r.get::<String, _>("data")).collect())
    }

    /// Reemplaza la tabla de usuarios por la lista dada (en una transacción)
    pub async fn replace_tg_users(&self, users: &[(String, String)]) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        sqlx::query(&format!("DELETE FROM {}", self.t("tg_users"))).execute(&mut *tx).await?;
        for (chat_id, data) in users {
            sqlx::query(&format!("INSERT INTO {} (chat_id, data) VALUES (?, ?)", self.t("tg_users")))
                .bind(chat_id)
                .bind(data)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    pub async fn load_tg_payments_json(&self) -> Result<Vec<String>> {
        let rows = sqlx::query(&format!("SELECT data FROM {} ORDER BY at", self.t("tg_payments")))
            .fetch_all(&self.pool)
            .await?;
        Ok(rows.into_iter().map(|r| r.get::<String, _>("data")).collect())
    }

    pub async fn insert_tg_payment(&self, chat_id: &str, stars: u64, charge_id: &str, at: DateTime<Utc>, data: &str) -> Result<()> {
        sqlx::query(&format!(
            "INSERT IGNORE INTO {} (chat_id, stars, charge_id, at, data) VALUES (?, ?, ?, ?, ?)",
            self.t("tg_payments")
        ))
        .bind(chat_id)
        .bind(stars as i64)
        .bind(charge_id)
        .bind(at)
        .bind(data)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    // ---------------- Posiciones abiertas ----------------

    pub async fn load_positions(&self, strategy: &str) -> Result<Vec<(String, String)>> {
        let rows = sqlx::query(&format!("SELECT symbol, data FROM {} WHERE strategy = ?", self.t("open_positions")))
            .bind(strategy)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows.into_iter().map(|r| (r.get::<String, _>("symbol"), r.get::<String, _>("data"))).collect())
    }

    /// Reemplaza las posiciones abiertas de una estrategia (en una transacción)
    pub async fn replace_positions(&self, strategy: &str, positions: &[(String, String)]) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        sqlx::query(&format!("DELETE FROM {} WHERE strategy = ?", self.t("open_positions")))
            .bind(strategy)
            .execute(&mut *tx)
            .await?;
        for (symbol, data) in positions {
            sqlx::query(&format!(
                "INSERT INTO {} (strategy, symbol, data) VALUES (?, ?, ?)",
                self.t("open_positions")
            ))
            .bind(strategy)
            .bind(symbol)
            .bind(data)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    async fn count(&self, table: &str) -> Result<i64> {
        Ok(sqlx::query(&format!("SELECT COUNT(*) AS n FROM {}", self.t(table)))
            .fetch_one(&self.pool)
            .await?
            .get::<i64, _>("n"))
    }

    // ---------------- Migración desde los JSON ----------------

    /// Importa los archivos JSON de versiones anteriores (solo si la tabla está vacía)
    /// y los renombra a *.migrado para no importarlos dos veces.
    pub async fn migrate_from_files(&self, dir: &Path) -> Result<()> {
        let mark = |name: &str| {
            let from = dir.join(name);
            let _ = std::fs::rename(&from, dir.join(format!("{}.migrado", name)));
        };
        let read = |name: &str| std::fs::read_to_string(dir.join(name)).ok();

        if let Some(s) = read("signals_history.json") {
            if self.count("signals").await? == 0 {
                let recs: Vec<SignalRecord> = serde_json::from_str(&s).unwrap_or_default();
                for r in &recs {
                    self.insert_signal(r).await?;
                }
                info!("📦 Migradas {} señales a la BD", recs.len());
            }
            mark("signals_history.json");
        }
        if let Some(s) = read("equity_history.json") {
            if self.count("equity_snapshots").await? == 0 {
                let pts: Vec<EquityPoint> = serde_json::from_str(&s).unwrap_or_default();
                for p in &pts {
                    self.insert_equity(p).await?;
                }
                info!("📦 Migradas {} fotos de equity a la BD", pts.len());
            }
            mark("equity_history.json");
        }
        if let Some(s) = read("subscribers.json") {
            if self.count("tg_users").await? == 0 {
                let users: Vec<serde_json::Value> = serde_json::from_str(&s).unwrap_or_default();
                let rows: Vec<(String, String)> = users
                    .iter()
                    .filter_map(|u| Some((u["chat_id"].as_str()?.to_string(), u.to_string())))
                    .collect();
                self.replace_tg_users(&rows).await?;
                info!("📦 Migrados {} usuarios de Telegram a la BD", rows.len());
            }
            mark("subscribers.json");
        }
        if let Some(s) = read("payments.json") {
            if self.count("tg_payments").await? == 0 {
                let pays: Vec<serde_json::Value> = serde_json::from_str(&s).unwrap_or_default();
                for p in &pays {
                    let at = p["at"].as_str().and_then(|a| a.parse::<DateTime<Utc>>().ok()).unwrap_or_else(Utc::now);
                    self.insert_tg_payment(
                        p["chat_id"].as_str().unwrap_or_default(),
                        p["stars"].as_u64().unwrap_or(0),
                        p["charge_id"].as_str().unwrap_or_default(),
                        at,
                        &p.to_string(),
                    )
                    .await?;
                }
                info!("📦 Migrados {} pagos a la BD", pays.len());
            }
            mark("payments.json");
        }
        for (file, strategy) in [("rebound_positions.json", "rebote"), ("trend_paper_positions.json", "tendencia_papel")] {
            if let Some(s) = read(file) {
                if self.load_positions(strategy).await?.is_empty() {
                    let map: std::collections::HashMap<String, serde_json::Value> =
                        serde_json::from_str(&s).unwrap_or_default();
                    let rows: Vec<(String, String)> = map.into_iter().map(|(k, v)| (k, v.to_string())).collect();
                    self.replace_positions(strategy, &rows).await?;
                    info!("📦 Migradas {} posiciones de {} a la BD", rows.len(), strategy);
                }
                mark(file);
            }
        }
        Ok(())
    }
}
