//! Señales de trading para suscriptores: eventos, historial persistente y track record.
//!
//! La estrategia publica `SignalEvent` en el notificador; el bot de Telegram las
//! reparte según el plan de cada usuario (premium en tiempo real, free con retraso)
//! y las guarda en `signals_history.json` para el historial público de resultados.

use chrono::{DateTime, Utc};
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tracing::{error, warn};

pub const DISCLAIMER: &str =
    "<i>⚠️ No es asesoría financiera. Operar con apalancamiento puede hacerte perder todo tu capital. Rendimientos pasados no garantizan resultados futuros.</i>";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum SignalEvent {
    Entry {
        symbol: String,
        price: Decimal,
        stop: Decimal,
        paper: bool,
        /// "tendencia" (vacío en historiales antiguos) o "rebote"
        #[serde(default)]
        strategy: String,
    },
    StopMoved {
        symbol: String,
        stop: Decimal,
        paper: bool,
    },
    Exit {
        symbol: String,
        entry: Decimal,
        exit: Decimal,
        /// Stop inicial (para medir el resultado en R = múltiplos del riesgo)
        initial_stop: Option<Decimal>,
        reason: String,
        paper: bool,
        #[serde(default)]
        strategy: String,
    },
}

pub const STRATEGY_TREND: &str = "tendencia";
pub const STRATEGY_REBOUND: &str = "rebote";

impl SignalEvent {
    pub fn symbol(&self) -> &str {
        match self {
            SignalEvent::Entry { symbol, .. }
            | SignalEvent::StopMoved { symbol, .. }
            | SignalEvent::Exit { symbol, .. } => symbol,
        }
    }

    /// Estrategia que generó la señal ("tendencia" o "rebote")
    pub fn strategy(&self) -> &str {
        match self {
            SignalEvent::Entry { strategy, .. } | SignalEvent::Exit { strategy, .. }
                if strategy == STRATEGY_REBOUND =>
            {
                STRATEGY_REBOUND
            }
            _ => STRATEGY_TREND,
        }
    }

    fn is_rebound(&self) -> bool {
        self.strategy() == STRATEGY_REBOUND
    }

    /// Resultado en % del precio (solo salidas, posiciones largas)
    pub fn pnl_pct(&self) -> Option<f64> {
        match self {
            SignalEvent::Exit { entry, exit, .. } if !entry.is_zero() => {
                ((*exit - *entry) / *entry * Decimal::from(100)).to_f64()
            }
            _ => None,
        }
    }

    /// Resultado en R: ganancia / riesgo inicial. Con 1% de riesgo por señal, 1R = 1% de la cuenta.
    pub fn r_multiple(&self) -> Option<f64> {
        match self {
            SignalEvent::Exit { entry, exit, initial_stop: Some(stop), .. } if *entry > *stop => {
                ((*exit - *entry) / (*entry - *stop)).to_f64()
            }
            _ => None,
        }
    }

    /// Mensaje completo (premium)
    pub fn format_premium(&self) -> String {
        let tag = |paper: bool| if paper { " <i>(papel)</i>" } else { "" };
        match self {
            SignalEvent::Entry { symbol, price, stop, paper, .. } => {
                let dist = if price.is_zero() {
                    0.0
                } else {
                    ((*price - *stop) / *price * Decimal::from(100)).to_f64().unwrap_or(0.0)
                };
                let (title, exit_rule) = if self.is_rebound() {
                    (
                        "🔄 <b>SEÑAL PREMIUM: REBOTE",
                        "🎯 Salida: al rebotar (RSI &gt; 50) o en máx. 48 h; te avisaremos",
                    )
                } else {
                    (
                        "🚀 <b>SEÑAL PREMIUM: COMPRA",
                        "🎯 Salida: stop dinámico (te avisaremos cuando subirlo)",
                    )
                };
                format!(
                    "{} {}</b>{}\n\n\
                     💵 Entrada: <b>${}</b>\n\
                     🛑 Stop: <b>${}</b> ({:.2}% abajo)\n\
                     {}\n\n\
                     📐 <b>Tamaño sugerido:</b> arriesga máx. 1% de tu capital.\n\
                     Tamaño = capital × 1% ÷ {:.2}%\n\
                     Ej.: con $1.000 → posición de ${:.0}\n\n{}",
                    title,
                    symbol,
                    tag(*paper),
                    price.normalize(),
                    stop.round_dp(6).normalize(),
                    dist,
                    exit_rule,
                    dist,
                    if dist > 0.0 { 10.0 / (dist / 100.0) } else { 0.0 },
                    DISCLAIMER
                )
            }
            SignalEvent::StopMoved { symbol, stop, paper } => format!(
                "🛡️ <b>{}: sube el stop a ${}</b>{}\nProtege la ganancia de la posición abierta.",
                symbol,
                stop.round_dp(6).normalize(),
                tag(*paper)
            ),
            SignalEvent::Exit { symbol, entry, exit, reason, paper, .. } => {
                let pct = self.pnl_pct().unwrap_or(0.0);
                let r = self
                    .r_multiple()
                    .map(|r| format!(" | {:+.2}R", r))
                    .unwrap_or_default();
                format!(
                    "{} <b>CIERRE {}{}</b>{}\n\n\
                     Entrada ${} → Salida ${}\n\
                     Resultado: <b>{:+.2}%</b>{}\n\
                     Motivo: {}",
                    if pct >= 0.0 { "✅" } else { "🔻" },
                    symbol,
                    if self.is_rebound() { " (rebote)" } else { "" },
                    tag(*paper),
                    entry.round_dp(6).normalize(),
                    exit.round_dp(6).normalize(),
                    pct,
                    r,
                    reason
                )
            }
        }
    }

    /// Aviso inmediato para usuarios free (sin precios)
    pub fn format_free_teaser(&self, delay_hours: i64) -> Option<String> {
        match self {
            SignalEvent::Entry { symbol, .. } => Some(format!(
                "🔒 <b>Nueva señal{} en {}</b>\n\n\
                 Los usuarios Premium ya tienen entrada, stop y tamaño sugerido.\n\
                 Tú la verás en {} h.\n\n\
                 👉 /premium para recibirlas al instante",
                if self.is_rebound() { " de rebote" } else { "" },
                symbol,
                delay_hours
            )),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignalRecord {
    pub id: u64,
    pub at: DateTime<Utc>,
    pub event: SignalEvent,
    /// Ya enviada (con retraso) a los usuarios free
    #[serde(default)]
    pub free_released: bool,
}

/// Resumen de resultados para /performance y el dashboard
#[derive(Debug, Clone, Default, Serialize)]
pub struct Performance {
    pub closed: usize,
    pub wins: usize,
    pub win_rate: f64,
    pub avg_win_pct: f64,
    pub avg_loss_pct: f64,
    pub total_r: f64,
    pub best_pct: f64,
    pub worst_pct: f64,
    pub open: usize,
    pub includes_paper: bool,
    pub since: Option<DateTime<Utc>>,
}

pub struct SignalHistory {
    path: PathBuf,
    pub records: Vec<SignalRecord>,
}

impl SignalHistory {
    pub fn load(path: &Path) -> Self {
        let records = std::fs::read_to_string(path)
            .ok()
            .and_then(|s| match serde_json::from_str::<Vec<SignalRecord>>(&s) {
                Ok(r) => Some(r),
                Err(e) => {
                    warn!("⚠️ No se pudo leer {:?}: {}", path, e);
                    None
                }
            })
            .unwrap_or_default();
        Self { path: path.to_path_buf(), records }
    }

    pub fn push(&mut self, event: SignalEvent) -> SignalRecord {
        let id = self.records.last().map_or(1, |r| r.id + 1);
        let rec = SignalRecord { id, at: Utc::now(), event, free_released: false };
        self.records.push(rec.clone());
        self.save();
        rec
    }

    pub fn save(&self) {
        match serde_json::to_string_pretty(&self.records) {
            Ok(s) => {
                if let Err(e) = std::fs::write(&self.path, s) {
                    error!("❌ Error guardando historial de señales: {}", e);
                }
            }
            Err(e) => error!("❌ Error serializando señales: {}", e),
        }
    }

    pub fn performance(&self) -> Performance {
        self.performance_for(None)
    }

    /// Rendimiento de una estrategia ("tendencia" / "rebote") o de todas (`None`)
    pub fn performance_for(&self, strategy: Option<&str>) -> Performance {
        let records: Vec<&SignalRecord> = self
            .records
            .iter()
            .filter(|r| strategy.map_or(true, |s| r.event.strategy() == s))
            .collect();
        let exits: Vec<&SignalEvent> = records
            .iter()
            .map(|r| &r.event)
            .filter(|e| matches!(e, SignalEvent::Exit { .. }))
            .collect();
        let pcts: Vec<f64> = exits.iter().filter_map(|e| e.pnl_pct()).collect();
        let wins: Vec<f64> = pcts.iter().copied().filter(|p| *p > 0.0).collect();
        let losses: Vec<f64> = pcts.iter().copied().filter(|p| *p <= 0.0).collect();
        let entries = records
            .iter()
            .filter(|r| matches!(r.event, SignalEvent::Entry { .. }))
            .count();
        let avg = |v: &[f64]| if v.is_empty() { 0.0 } else { v.iter().sum::<f64>() / v.len() as f64 };

        Performance {
            closed: pcts.len(),
            wins: wins.len(),
            win_rate: if pcts.is_empty() { 0.0 } else { wins.len() as f64 / pcts.len() as f64 },
            avg_win_pct: avg(&wins),
            avg_loss_pct: avg(&losses),
            total_r: exits.iter().filter_map(|e| e.r_multiple()).sum(),
            best_pct: pcts.iter().copied().fold(0.0, f64::max),
            worst_pct: pcts.iter().copied().fold(0.0, f64::min),
            open: entries.saturating_sub(exits.len()),
            includes_paper: records.iter().any(|r| match &r.event {
                SignalEvent::Entry { paper, .. }
                | SignalEvent::StopMoved { paper, .. }
                | SignalEvent::Exit { paper, .. } => *paper,
            }),
            since: records.first().map(|r| r.at),
        }
    }
}

impl Performance {
    /// Bloque compacto con título, sin aviso legal (para combinar estrategias)
    pub fn format_section(&self, title: &str) -> String {
        if self.closed == 0 {
            return format!("<b>{}</b>\nSin señales cerradas aún ({} abiertas).\n", title, self.open);
        }
        format!(
            "<b>{}</b>{}\n\
             Cerradas: {} (abiertas: {}) · aciertos {:.0}%\n\
             Ganancia media {:+.2}% · pérdida media {:+.2}%\n\
             Total: <b>{:+.1}R</b>\n",
            title,
            if self.includes_paper { " <i>(papel)</i>" } else { "" },
            self.closed,
            self.open,
            self.win_rate * 100.0,
            self.avg_win_pct,
            self.avg_loss_pct,
            self.total_r
        )
    }

    pub fn format(&self) -> String {
        if self.closed == 0 {
            return format!(
                "📊 <b>HISTORIAL DE SEÑALES</b>\n\nAún no hay señales cerradas ({} abiertas).\n\n{}",
                self.open, DISCLAIMER
            );
        }
        format!(
            "📊 <b>HISTORIAL DE SEÑALES</b>{}\n\
             Desde: {}\n\n\
             Señales cerradas: <b>{}</b> (abiertas: {})\n\
             Aciertos: <b>{:.0}%</b> ({} de {})\n\
             Ganancia media: <b>{:+.2}%</b> | Pérdida media: <b>{:+.2}%</b>\n\
             Mejor: {:+.2}% | Peor: {:+.2}%\n\
             Total: <b>{:+.1}R</b> (≈ {:+.1}% de cuenta arriesgando 1% por señal)\n\n{}",
            if self.includes_paper { " <i>(incluye señales en papel)</i>" } else { "" },
            self.since.map(|d| d.format("%d-%m-%Y").to_string()).unwrap_or_default(),
            self.closed,
            self.open,
            self.win_rate * 100.0,
            self.wins,
            self.closed,
            self.avg_win_pct,
            self.avg_loss_pct,
            self.best_pct,
            self.worst_pct,
            self.total_r,
            self.total_r,
            DISCLAIMER
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn exit(entry: Decimal, exit: Decimal, stop: Decimal) -> SignalEvent {
        SignalEvent::Exit {
            symbol: "BTC".into(),
            entry,
            exit,
            initial_stop: Some(stop),
            reason: "test".into(),
            paper: true,
            strategy: String::new(),
        }
    }

    #[test]
    fn r_multiple_and_pct() {
        let e = exit(dec!(100), dec!(106), dec!(97));
        assert!((e.r_multiple().unwrap() - 2.0).abs() < 1e-9);
        assert!((e.pnl_pct().unwrap() - 6.0).abs() < 1e-9);
    }

    #[test]
    fn performance_counts() {
        let dir = std::env::temp_dir().join(format!("sig_test_{}.json", std::process::id()));
        let mut h = SignalHistory::load(&dir);
        h.push(SignalEvent::Entry { symbol: "BTC".into(), price: dec!(100), stop: dec!(97), paper: true, strategy: String::new() });
        h.push(exit(dec!(100), dec!(106), dec!(97)));
        h.push(SignalEvent::Entry { symbol: "ETH".into(), price: dec!(10), stop: dec!(9), paper: true, strategy: STRATEGY_REBOUND.into() });
        h.push(exit(dec!(10), dec!(9), dec!(9)));
        let p = h.performance();
        assert_eq!(p.closed, 2);
        assert_eq!(p.wins, 1);
        assert!((p.total_r - 1.0).abs() < 1e-9);
        // Por estrategia: los cierres del test no tienen strategy -> cuentan como tendencia
        assert_eq!(h.performance_for(Some(STRATEGY_TREND)).closed, 2);
        // La entrada de ETH es de rebote y no tiene cierre propio -> 1 abierta
        assert_eq!(h.performance_for(Some(STRATEGY_REBOUND)).open, 1);
        assert_eq!(h.performance_for(Some(STRATEGY_REBOUND)).closed, 0);
        // Historial antiguo sin el campo strategy se sigue leyendo
        let old: SignalEvent = serde_json::from_str(
            r#"{"kind":"Entry","symbol":"BTC","price":"1","stop":"0.9","paper":true}"#,
        )
        .unwrap();
        assert_eq!(old.strategy(), STRATEGY_TREND);
        let _ = std::fs::remove_file(dir);
    }
}
