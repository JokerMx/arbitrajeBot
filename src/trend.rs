//! Estrategia de seguimiento de tendencia: ruptura Donchian en velas de 1h, solo largos.
//!
//! Reglas (validadas en tools/backtest/trend.py y portfolio.py):
//! - Entrada: el cierre de la última vela de 1h supera el máximo de las 168 velas previas (7 días).
//! - Stop inicial: entrada - 3 ATR(14). Trailing: max(stop, cierre - 3 ATR) en cada vela cerrada.
//! - Salida por canal: el cierre cae bajo el mínimo de las 24 velas previas.
//! - Tamaño: se arriesga un % fijo del equity hasta el stop; tope de apalancamiento total y
//!   número máximo de posiciones simultáneas.
//! - El stop vive en el EXCHANGE (trigger reduce-only): protege aunque el bot se caiga.
//!
//! Con `live = false` opera en papel: mismas señales, posiciones virtuales, sin órdenes.

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use tracing::{error, info, warn};

use crate::hyperliquid::client::STOP_SLIPPAGE;
use crate::hyperliquid::{HyperliquidClient, HyperliquidOrderSide};
use crate::notifier::TelegramNotifier;
use crate::risk::RiskManager;
use crate::signals::{SignalEvent, STRATEGY_REBOUND, STRATEGY_TREND};

const HOUR_MS: u64 = 3_600_000;
/// Espera tras el cierre de la vela antes de evaluar (la API tarda en publicarla)
const EVAL_DELAY_MS: u64 = 20_000;
/// Fee taker + slippage estimado (para el P&L en papel)
const PAPER_COST: Decimal = dec!(0.00095);
/// Mínimo de Hyperliquid por orden
const MIN_ORDER_USD: Decimal = dec!(10);

/// Monedas definidas en TREND_SYMBOLS (valor por defecto de la selección)
pub fn env_symbols() -> Vec<String> {
    std::env::var("TREND_SYMBOLS")
        .unwrap_or_default()
        .split(',')
        .map(|s| s.trim().to_uppercase())
        .filter(|s| !s.is_empty())
        .collect()
}

#[derive(Debug, Clone, PartialEq)]
pub struct TrendConfig {
    /// Velas de 1h del canal de entrada
    pub breakout_hours: usize,
    /// Velas de 1h del canal de salida
    pub exit_hours: usize,
    pub atr_period: usize,
    /// Distancia del stop en ATRs
    pub stop_atr: Decimal,
    /// Fracción del equity arriesgada por operación (0.01 = 1%)
    pub risk_per_trade: Decimal,
    /// Riesgo máximo aceptado al subir una posición pequeña al mínimo de $10.
    /// Con cuentas chicas las monedas volátiles salen bajo el mínimo; si al abrir
    /// $10 el riesgo real no pasa de este tope, se opera igual.
    pub max_risk_per_trade: Decimal,
    /// Nocional total máximo / equity
    pub max_total_leverage: Decimal,
    pub max_positions: usize,
    /// Precio límite de la entrada IOC sobre el mid
    pub entry_slippage: Decimal,
    /// No entrar si el funding medio de 24 h supera 2x el normal (mercado sobrecomprado).
    /// Validado en tools/backtest/trend_filters.py: mejora ambas mitades.
    pub funding_filter: bool,
}

/// Funding "normal" de Hyperliquid por hora (0,00125% ≈ 11% anual)
pub const BASE_FUNDING_HOURLY: Decimal = dec!(0.0000125);

impl Default for TrendConfig {
    fn default() -> Self {
        Self {
            breakout_hours: 168,
            exit_hours: 24,
            atr_period: 14,
            stop_atr: dec!(3),
            // Backtest: 0.5% -> DD máx ~7%; 1% -> DD ~20%; 2% -> DD ~44% (no usar)
            risk_per_trade: dec!(0.01),
            max_risk_per_trade: dec!(0.0125),
            max_total_leverage: dec!(3),
            max_positions: 4,
            entry_slippage: dec!(0.005),
            funding_filter: true,
        }
    }
}

impl TrendConfig {
    /// Valores por defecto sobrescribibles por variables de entorno (TREND_*)
    pub fn from_env() -> Self {
        fn var<T: std::str::FromStr>(key: &str) -> Option<T> {
            std::env::var(key).ok().and_then(|v| v.trim().parse().ok())
        }
        let d = Self::default();
        let cfg = Self {
            breakout_hours: var("TREND_BREAKOUT_HOURS").unwrap_or(d.breakout_hours),
            exit_hours: var("TREND_EXIT_HOURS").unwrap_or(d.exit_hours),
            atr_period: d.atr_period,
            stop_atr: var("TREND_STOP_ATR").unwrap_or(d.stop_atr),
            risk_per_trade: var("TREND_RISK_PER_TRADE").unwrap_or(d.risk_per_trade),
            max_risk_per_trade: var("TREND_MAX_RISK_PER_TRADE").unwrap_or(d.max_risk_per_trade),
            max_total_leverage: var("TREND_MAX_LEVERAGE").unwrap_or(d.max_total_leverage),
            max_positions: var("TREND_MAX_POSITIONS").unwrap_or(d.max_positions),
            entry_slippage: d.entry_slippage,
            funding_filter: var("TREND_FUNDING_FILTER").unwrap_or(d.funding_filter),
        };
        cfg.sanitized()
    }

    /// Límites de seguridad (el backtest mostró caídas de ~44% con 2% de riesgo)
    pub fn sanitized(self) -> Self {
        let risk_per_trade = self.risk_per_trade.max(dec!(0.001)).min(dec!(0.02));
        Self {
            risk_per_trade,
            // Nunca menor que el riesgo normal ni mayor que 2%
            max_risk_per_trade: self.max_risk_per_trade.max(risk_per_trade).min(dec!(0.02)),
            max_total_leverage: self.max_total_leverage.max(Decimal::ONE).min(dec!(5)),
            max_positions: self.max_positions.clamp(1, 10),
            breakout_hours: self.breakout_hours.clamp(24, 1000),
            exit_hours: self.exit_hours.clamp(6, 500),
            stop_atr: self.stop_atr.max(dec!(1)).min(dec!(6)),
            ..self
        }
    }
}

/// Ajustes de estrategia hechos desde el dashboard (se guardan en la BD como JSON
/// y se aplican sobre los valores del .env). Todos opcionales.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StrategyOverrides {
    pub trend_risk_per_trade: Option<f64>,
    pub trend_max_risk_per_trade: Option<f64>,
    pub trend_max_positions: Option<usize>,
    pub trend_max_leverage: Option<f64>,
    pub trend_breakout_hours: Option<usize>,
    pub trend_exit_hours: Option<usize>,
    pub trend_stop_atr: Option<f64>,
    pub trend_funding_filter: Option<bool>,
    pub rebound_enabled: Option<bool>,
    pub rebound_entry_rsi: Option<f64>,
    pub rebound_exit_rsi: Option<f64>,
    pub rebound_max_hold_hours: Option<u64>,
    pub rebound_risk_per_trade: Option<f64>,
    pub rebound_max_positions: Option<usize>,
}

impl StrategyOverrides {
    /// Configuración efectiva: .env + ajustes del dashboard, con límites de seguridad.
    /// `rebound.live` nunca se cambia desde aquí (operar real solo por .env).
    pub fn apply(&self, trend: &TrendConfig, rebound: &ReboundConfig) -> (TrendConfig, ReboundConfig) {
        use rust_decimal::prelude::FromPrimitive;
        let d = |v: Option<f64>, cur: Decimal| v.and_then(Decimal::from_f64).unwrap_or(cur);
        let t = TrendConfig {
            risk_per_trade: d(self.trend_risk_per_trade, trend.risk_per_trade),
            max_risk_per_trade: d(self.trend_max_risk_per_trade, trend.max_risk_per_trade),
            max_positions: self.trend_max_positions.unwrap_or(trend.max_positions),
            max_total_leverage: d(self.trend_max_leverage, trend.max_total_leverage),
            breakout_hours: self.trend_breakout_hours.unwrap_or(trend.breakout_hours),
            exit_hours: self.trend_exit_hours.unwrap_or(trend.exit_hours),
            stop_atr: d(self.trend_stop_atr, trend.stop_atr),
            funding_filter: self.trend_funding_filter.unwrap_or(trend.funding_filter),
            ..trend.clone()
        }
        .sanitized();
        let r = ReboundConfig {
            enabled: self.rebound_enabled.unwrap_or(rebound.enabled),
            entry_rsi: d(self.rebound_entry_rsi, rebound.entry_rsi),
            exit_rsi: d(self.rebound_exit_rsi, rebound.exit_rsi),
            max_hold_hours: self.rebound_max_hold_hours.unwrap_or(rebound.max_hold_hours),
            risk_per_trade: d(self.rebound_risk_per_trade, rebound.risk_per_trade),
            max_positions: self.rebound_max_positions.unwrap_or(rebound.max_positions),
            live: rebound.live,
            ..rebound.clone()
        }
        .sanitized();
        (t, r)
    }
}

/// Nocional de una entrada, o `None` si no se puede operar.
/// - `stop_frac`: distancia al stop como fracción del precio.
/// - `room`: nocional disponible bajo el tope de apalancamiento.
/// Si el tamaño por riesgo queda bajo el mínimo del exchange, se usa el mínimo
/// siempre que su riesgo real no supere `max_risk` y quepa en `room`.
fn entry_notional(
    equity: Decimal,
    risk: Decimal,
    max_risk: Decimal,
    stop_frac: Decimal,
    room: Decimal,
) -> Option<Decimal> {
    if stop_frac <= Decimal::ZERO || equity <= Decimal::ZERO {
        return None;
    }
    let by_risk = equity * risk / stop_frac;
    let notional = by_risk.min(room);
    if notional >= MIN_ORDER_USD {
        return Some(notional);
    }
    let risk_at_min = MIN_ORDER_USD * stop_frac;
    if room >= MIN_ORDER_USD && risk_at_min <= equity * max_risk {
        Some(MIN_ORDER_USD)
    } else {
        None
    }
}

/// Estrategia complementaria "rebote": compra una moneda en sobreventa extrema
/// (RSI 1h < 20) y vende al rebotar (RSI > 50) o tras 48 h. Validada en
/// tools/backtest/dips.py (positiva en ambas mitades, pero con pocas operaciones):
/// por eso corre en papel salvo REBOUND_LIVE=true.
#[derive(Debug, Clone, PartialEq)]
pub struct ReboundConfig {
    pub enabled: bool,
    /// Operar con dinero real (solo si además el bot está en hyperliquid_mainnet)
    pub live: bool,
    pub entry_rsi: Decimal,
    pub exit_rsi: Decimal,
    pub max_hold_hours: u64,
    pub stop_atr: Decimal,
    pub risk_per_trade: Decimal,
    pub max_positions: usize,
}

impl Default for ReboundConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            live: false,
            entry_rsi: dec!(20),
            exit_rsi: dec!(50),
            max_hold_hours: 48,
            stop_atr: dec!(3),
            risk_per_trade: dec!(0.005),
            max_positions: 2,
        }
    }
}

impl ReboundConfig {
    pub fn from_env() -> Self {
        fn var<T: std::str::FromStr>(key: &str) -> Option<T> {
            std::env::var(key).ok().and_then(|v| v.trim().parse().ok())
        }
        let d = Self::default();
        Self {
            enabled: var("REBOUND_ENABLED").unwrap_or(d.enabled),
            live: var("REBOUND_LIVE").unwrap_or(d.live),
            entry_rsi: var::<Decimal>("REBOUND_ENTRY_RSI").unwrap_or(d.entry_rsi),
            exit_rsi: var::<Decimal>("REBOUND_EXIT_RSI").unwrap_or(d.exit_rsi),
            max_hold_hours: var::<u64>("REBOUND_MAX_HOLD_HOURS").unwrap_or(d.max_hold_hours),
            stop_atr: d.stop_atr,
            risk_per_trade: var::<Decimal>("REBOUND_RISK_PER_TRADE").unwrap_or(d.risk_per_trade),
            max_positions: var::<usize>("REBOUND_MAX_POSITIONS").unwrap_or(d.max_positions),
        }
        .sanitized()
    }

    pub fn sanitized(self) -> Self {
        Self {
            entry_rsi: self.entry_rsi.max(dec!(5)).min(dec!(40)),
            exit_rsi: self.exit_rsi.max(dec!(30)).min(dec!(80)),
            max_hold_hours: self.max_hold_hours.clamp(4, 240),
            risk_per_trade: self.risk_per_trade.max(dec!(0.001)).min(dec!(0.02)),
            max_positions: self.max_positions.clamp(0, 5),
            ..self
        }
    }
}

/// Posición de la estrategia de rebote (se guarda en DATA_DIR/rebound_positions.json)
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ReboundPosition {
    size: Decimal,
    entry: Decimal,
    stop: Decimal,
    opened_ms: u64,
    paper: bool,
    stop_oid: Option<u64>,
}

#[derive(Debug, Clone)]
struct Candle {
    t: u64,
    high: Decimal,
    low: Decimal,
    close: Decimal,
}

#[derive(Deserialize)]
struct RawCandle {
    t: u64,
    #[serde(rename = "T")]
    close_time: u64,
    h: String,
    l: String,
    c: String,
}

#[derive(Debug, Clone)]
struct Signal {
    candle_t: u64,
    close: Decimal,
    atr: Decimal,
    breakout_high: Decimal,
    exit_low: Decimal,
    /// RSI(14) de Wilder sobre los cierres de 1h
    rsi: Decimal,
}

impl Signal {
    fn is_breakout(&self) -> bool {
        self.close > self.breakout_high
    }
    /// Fuerza de la ruptura en ATRs (para priorizar cuando hay pocos cupos)
    fn strength(&self) -> Decimal {
        if self.atr.is_zero() {
            Decimal::ZERO
        } else {
            (self.close - self.breakout_high) / self.atr
        }
    }
}

#[derive(Debug, Clone)]
struct PaperPosition {
    size: Decimal,
    entry: Decimal,
}

#[derive(Debug, Clone, Default)]
struct CoinState {
    stop: Option<Decimal>,
    stop_oid: Option<u64>,
    paper: Option<PaperPosition>,
    /// Precio de entrada y stop inicial (para informar resultados a suscriptores)
    entry: Option<Decimal>,
    initial_stop: Option<Decimal>,
}

#[derive(Clone)]
pub struct TrendTrader {
    cfg: TrendConfig,
    /// Monedas seleccionadas; compartidas con la API para cambiarlas sin reiniciar
    symbols: Arc<RwLock<Vec<String>>>,
    client: Arc<HyperliquidClient>,
    risk_manager: Arc<Mutex<RiskManager>>,
    notifier: Arc<TelegramNotifier>,
    http: reqwest::Client,
    info_url: &'static str,
    live: bool,
    states: HashMap<String, CoinState>,
    last_eval_hour: u64,
    paper_equity: Decimal,
    halted: bool,
    rebound_cfg: ReboundConfig,
    rebound: HashMap<String, ReboundPosition>,
    rebound_file: std::path::PathBuf,
    /// Configuración editable desde el dashboard (se aplica en cada evaluación)
    shared_cfg: Arc<RwLock<(TrendConfig, ReboundConfig)>>,
    /// Pausa de nuevas entradas (las posiciones abiertas se siguen gestionando)
    paused: Arc<AtomicBool>,
}

impl TrendTrader {
    pub fn new(
        cfg: TrendConfig,
        symbols: Vec<String>,
        client: Arc<HyperliquidClient>,
        risk_manager: Arc<Mutex<RiskManager>>,
        notifier: Arc<TelegramNotifier>,
        live: bool,
        mainnet: bool,
        initial_capital: Decimal,
        rebound_cfg: ReboundConfig,
        paused: bool,
    ) -> Self {
        let states = symbols.iter().map(|s| (s.clone(), CoinState::default())).collect();
        let cfg_for_share = cfg.clone();
        let data_dir = std::env::var("DATA_DIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|_| std::env::current_dir().unwrap_or_default());
        let rebound_file = data_dir.join("rebound_positions.json");
        let rebound: HashMap<String, ReboundPosition> = std::fs::read_to_string(&rebound_file)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        if !rebound.is_empty() {
            info!("♻️ Rebote: {} posiciones recuperadas de {:?}", rebound.len(), rebound_file);
        }
        Self {
            cfg,
            symbols: Arc::new(RwLock::new(symbols)),
            client,
            risk_manager,
            notifier,
            http: reqwest::Client::new(),
            info_url: if mainnet {
                "https://api.hyperliquid.xyz/info"
            } else {
                "https://api.hyperliquid-testnet.xyz/info"
            },
            live,
            states,
            last_eval_hour: 0,
            paper_equity: initial_capital,
            halted: false,
            shared_cfg: Arc::new(RwLock::new((cfg_for_share, rebound_cfg.clone()))),
            paused: Arc::new(AtomicBool::new(paused)),
            rebound_cfg,
            rebound,
            rebound_file,
        }
    }

    pub fn symbols_handle(&self) -> Arc<RwLock<Vec<String>>> {
        self.symbols.clone()
    }

    pub fn config_handle(&self) -> Arc<RwLock<(TrendConfig, ReboundConfig)>> {
        self.shared_cfg.clone()
    }

    pub fn paused_handle(&self) -> Arc<AtomicBool> {
        self.paused.clone()
    }

    pub fn is_live(&self) -> bool {
        self.live
    }

    pub fn client(&self) -> Arc<HyperliquidClient> {
        self.client.clone()
    }

    pub fn available_markets(&self) -> Vec<String> {
        self.client.market_names()
    }

    pub async fn start(&mut self) -> anyhow::Result<()> {
        let symbols = self.symbols.read().await.clone();
        info!(
            "📈 Estrategia TENDENCIA ({}) en {:?}: ruptura {}h, salida {}h, stop {} ATR, riesgo {}%/op (hasta {}% al subir a $10), máx {} posiciones, máx {}x",
            if self.live { "REAL" } else { "PAPEL" },
            symbols,
            self.cfg.breakout_hours,
            self.cfg.exit_hours,
            self.cfg.stop_atr,
            self.cfg.risk_per_trade * dec!(100),
            self.cfg.max_risk_per_trade * dec!(100),
            self.cfg.max_positions,
            self.cfg.max_total_leverage
        );

        if self.rebound_cfg.enabled {
            info!(
                "🔄 Estrategia REBOTE ({}): compra con RSI < {}, vende con RSI > {} o a las {} h, stop {} ATR, riesgo {}%/op, máx {} posiciones",
                if self.rebound_is_live() { "REAL" } else { "PAPEL" },
                self.rebound_cfg.entry_rsi,
                self.rebound_cfg.exit_rsi,
                self.rebound_cfg.max_hold_hours,
                self.rebound_cfg.stop_atr,
                self.rebound_cfg.risk_per_trade * dec!(100),
                self.rebound_cfg.max_positions
            );
        }

        if self.live {
            // Recuperar stops existentes en el exchange (reinicio del bot)
            let (_, positions) = self.client.get_account_snapshot().await?;
            for symbol in symbols.clone() {
                if !positions.contains_key(&symbol) {
                    continue;
                }
                match self.client.get_stop_orders(&symbol).await {
                    Ok(stops) if !stops.is_empty() => {
                        let (oid, limit_px) = stops[stops.len() - 1];
                        let stop = limit_px / (Decimal::ONE - STOP_SLIPPAGE);
                        info!("♻️ {}: stop recuperado oid={} ~${:.4}", symbol, oid, stop);
                        let st = self.states.entry(symbol.clone()).or_default();
                        st.stop = Some(stop);
                        st.stop_oid = Some(oid);
                    }
                    Ok(_) => warn!("⚠️ {}: posición SIN stop en el exchange; se colocará ahora", symbol),
                    Err(e) => warn!("No se pudieron leer stops de {}: {}", symbol, e),
                }
            }
        }

        // Evaluar de inmediato (coloca stops faltantes y busca entradas)
        self.evaluate_all().await;
        self.last_eval_hour = now_ms() / HOUR_MS;
        Ok(())
    }

    /// Llamado periódicamente (cada ~10 s) desde bot.rs
    pub async fn tick(&mut self) {
        if !self.live {
            self.check_paper_stops().await;
        }
        self.check_rebound_paper_stops().await;
        let now = now_ms();
        let hour = now / HOUR_MS;
        if hour != self.last_eval_hour && now % HOUR_MS >= EVAL_DELAY_MS {
            self.last_eval_hour = hour;
            self.refresh_config().await;
            self.evaluate_all().await;
        }
    }

    /// Toma la configuración editada desde el dashboard (no cambia el modo real/papel)
    async fn refresh_config(&mut self) {
        let (t, r) = self.shared_cfg.read().await.clone();
        let live = self.rebound_cfg.live;
        self.cfg = t;
        self.rebound_cfg = ReboundConfig { live, ..r };
    }

    pub async fn stop(&mut self) {
        if self.live {
            warn!("🛑 Bot detenido: las posiciones abiertas siguen protegidas por sus stops en el exchange");
        } else {
            info!("📄 Papel: equity final ${:.2}", self.paper_equity);
        }
    }

    // ------------------------------------------------------------------
    // Evaluación horaria
    // ------------------------------------------------------------------

    async fn evaluate_all(&mut self) {
        // 1. Estado de la cuenta (real o papel)
        let (equity, positions) = match self.account().await {
            Ok(x) => x,
            Err(e) => {
                error!("Error leyendo cuenta: {}", e);
                return;
            }
        };

        // 2. Límites globales del risk manager
        let flatten = {
            let mut rm = self.risk_manager.lock().await;
            rm.sync_balance(equity);
            rm.should_flatten()
        };
        if flatten {
            if !self.halted {
                self.halted = true;
                let msg = "🚨 Límite de riesgo alcanzado: cerrando todo y sin nuevas entradas";
                error!("{}", msg);
                let _ = self.notifier.send_message(msg).await;
                for symbol in positions.keys().cloned().collect::<Vec<_>>() {
                    self.exit(&symbol, "límite de riesgo").await;
                }
            }
            return;
        }

        // 3. Señales y gestión de posiciones abiertas
        let mut used_notional = Decimal::ZERO;
        let mut open_count = 0usize;
        let mut candidates: Vec<(String, Signal)> = Vec::new();
        let mut signals: HashMap<String, Signal> = HashMap::new();

        // Seleccionadas + las que el bot abrió y luego se quitaron de la selección:
        // esas se siguen gestionando (stop y salida) pero no se reabren.
        let selected = self.symbols.read().await.clone();
        let mut to_evaluate = selected.clone();
        for symbol in positions.keys() {
            let opened_by_bot = self
                .states
                .get(symbol)
                .map_or(false, |st| st.stop.is_some() || st.paper.is_some());
            if opened_by_bot && !to_evaluate.contains(symbol) {
                info!("ℹ️ {}: ya no está seleccionada; se gestiona la posición abierta hasta cerrarla", symbol);
                to_evaluate.push(symbol.clone());
            }
        }

        // Monedas con posición de rebote: también se evalúan para gestionar su salida
        for symbol in self.rebound.keys() {
            if !to_evaluate.contains(symbol) {
                to_evaluate.push(symbol.clone());
            }
        }

        for symbol in to_evaluate {
            let signal = match self.signal(&symbol).await {
                Ok(Some(s)) => s,
                Ok(None) => continue,
                Err(e) => {
                    warn!("Sin velas para {}: {}", symbol, e);
                    continue;
                }
            };
            signals.insert(symbol.clone(), signal.clone());

            // Las posiciones de rebote las gestiona evaluate_rebound, no la tendencia
            if let Some(rb) = self.rebound.get(&symbol) {
                used_notional += rb.size * signal.close;
                continue;
            }

            match positions.get(&symbol) {
                Some(&(size, entry)) if size > Decimal::ZERO => {
                    open_count += 1;
                    used_notional += size * signal.close;
                    self.manage_long(&symbol, size, entry, &signal).await;
                }
                Some(&(size, _)) if size < Decimal::ZERO => {
                    open_count += 1;
                    used_notional += size.abs() * signal.close;
                    warn!("⚠️ {}: posición corta {} abierta manualmente; la estrategia no la gestiona", symbol, size);
                }
                _ => {
                    self.clear_state(&symbol).await;
                    if signal.is_breakout() && selected.contains(&symbol) && !self.rebound.contains_key(&symbol) {
                        candidates.push((symbol.clone(), signal));
                    }
                }
            }
        }

        // 4. Entradas, las rupturas más fuertes primero
        candidates.sort_by(|a, b| b.1.strength().cmp(&a.1.strength()));
        let paused = self.paused.load(Ordering::Relaxed);
        for (symbol, signal) in candidates {
            if paused {
                info!("⏸️ {}: ruptura ignorada (bot en pausa)", symbol);
                continue;
            }
            if open_count >= self.cfg.max_positions {
                info!("⏸️ {}: ruptura ignorada, máximo de {} posiciones", symbol, self.cfg.max_positions);
                continue;
            }
            if let Some(notional) = self.enter(&symbol, &signal, equity, used_notional).await {
                open_count += 1;
                used_notional += notional;
            }
        }

        // 5. Estrategia de rebote (salidas y entradas)
        if self.rebound_cfg.enabled {
            let trend_held: Vec<String> = positions
                .iter()
                .filter(|(s, (size, _))| !size.is_zero() && !self.rebound.contains_key(*s))
                .map(|(s, _)| s.clone())
                .chain(self.states.iter().filter(|(_, st)| st.paper.is_some()).map(|(s, _)| s.clone()))
                .collect();
            used_notional = self
                .evaluate_rebound(&signals, &selected, &trend_held, &positions, equity, used_notional)
                .await;
        }

        info!(
            "📊 Tendencia: equity ${:.2}, {} posiciones (+{} rebote), nocional ${:.2} ({:.2}x)",
            equity,
            open_count,
            self.rebound.len(),
            used_notional,
            if equity > Decimal::ZERO { used_notional / equity } else { Decimal::ZERO }
        );
    }

    async fn manage_long(&mut self, symbol: &str, size: Decimal, entry: Decimal, sig: &Signal) {
        let dist = self.cfg.stop_atr * sig.atr;
        let prev_stop = self.states.get(symbol).and_then(|s| s.stop);
        let initial = prev_stop.unwrap_or(entry - dist);
        let new_stop = initial.max(sig.close - dist);

        if sig.close < sig.exit_low {
            self.exit(symbol, &format!("cierre ${:.4} bajo mínimo {}h ${:.4}", sig.close, self.cfg.exit_hours, sig.exit_low))
                .await;
            return;
        }
        if sig.close <= new_stop {
            self.exit(symbol, "precio bajo el stop").await;
            return;
        }

        {
            let st = self.states.entry(symbol.to_string()).or_default();
            st.entry.get_or_insert(entry);
        }

        let has_order = self.states.get(symbol).map_or(false, |s| s.stop_oid.is_some() || !self.live);
        let moved = prev_stop.map_or(true, |p| new_stop > p);
        if !moved && has_order {
            return;
        }

        if self.live {
            // Reemplazar el stop: primero el nuevo, luego cancelar el viejo
            // (la posición nunca queda sin protección)
            let old_oid = self.states.get(symbol).and_then(|s| s.stop_oid);
            match self.client.place_stop_loss(symbol, true, size, new_stop).await {
                Ok(oid) => {
                    if let (Some(old), Some(asset)) = (old_oid, self.asset(symbol)) {
                        if oid != Some(old) {
                            if let Err(e) = self.client.cancel_order(asset, old).await {
                                warn!("No se pudo cancelar stop anterior de {}: {}", symbol, e);
                            }
                        }
                    }
                    let st = self.states.entry(symbol.to_string()).or_default();
                    st.stop = Some(new_stop);
                    st.stop_oid = oid;
                    info!("🛡️ {}: stop en ${:.4} (cierre ${:.4})", symbol, new_stop, sig.close);
                    if prev_stop.is_some() && moved {
                        self.notifier.publish_signal(SignalEvent::StopMoved {
                            symbol: symbol.to_string(),
                            stop: new_stop,
                            paper: false,
                        });
                    }
                }
                Err(e) => {
                    // Nunca mantener una posición sin stop
                    error!("❌ {}: no se pudo colocar stop ({}); cerrando posición", symbol, e);
                    self.exit(symbol, "sin stop").await;
                }
            }
        } else {
            let st = self.states.entry(symbol.to_string()).or_default();
            st.stop = Some(new_stop);
            info!("📄 {}: stop (papel) en ${:.4}", symbol, new_stop);
            if prev_stop.is_some() && moved {
                self.notifier.publish_signal(SignalEvent::StopMoved {
                    symbol: symbol.to_string(),
                    stop: new_stop,
                    paper: true,
                });
            }
        }
    }

    /// Abre un largo. Devuelve el nocional abierto.
    async fn enter(&mut self, symbol: &str, sig: &Signal, equity: Decimal, used: Decimal) -> Option<Decimal> {
        let dist = self.cfg.stop_atr * sig.atr;
        if dist <= Decimal::ZERO || sig.close <= Decimal::ZERO {
            return None;
        }
        if self.cfg.funding_filter {
            match self.avg_funding_24h(symbol).await {
                Ok(f) if f > BASE_FUNDING_HOURLY * dec!(2) => {
                    info!(
                        "⏸️ {}: ruptura ignorada por funding alto ({:.4}%/h, {:.0}% anual; límite 2x normal)",
                        symbol,
                        f * dec!(100),
                        f * dec!(100) * dec!(8760)
                    );
                    return None;
                }
                Ok(_) => {}
                // Sin dato de funding no se bloquea la entrada
                Err(e) => warn!("{}: no se pudo leer el funding ({}); se ignora el filtro", symbol, e),
            }
        }
        let stop_frac = dist / sig.close;
        let room = equity * self.cfg.max_total_leverage - used;
        let notional = match entry_notional(
            equity,
            self.cfg.risk_per_trade,
            self.cfg.max_risk_per_trade,
            stop_frac,
            room,
        ) {
            Some(n) => n,
            None => {
                info!(
                    "⏸️ {}: ruptura sin tamaño suficiente (stop a {:.2}%: $10 arriesgaría {:.2}% > máx {}%; cupo ${:.2})",
                    symbol,
                    stop_frac * dec!(100),
                    MIN_ORDER_USD * stop_frac / equity * dec!(100),
                    self.cfg.max_risk_per_trade * dec!(100),
                    room
                );
                return None;
            }
        };
        let real_risk = notional * stop_frac / equity;
        if real_risk > self.cfg.risk_per_trade + dec!(0.00001) {
            info!(
                "↗️ {}: posición subida al mínimo de $10 (riesgo real {:.2}% en vez de {}%)",
                symbol,
                real_risk * dec!(100),
                self.cfg.risk_per_trade * dec!(100)
            );
        }
        let size = notional / sig.close;
        let stop = sig.close - dist;
        let msg = format!(
            "🚀 ENTRADA {} {}: ruptura de ${:.4} (máx {}h) a ${:.4}, nocional ${:.2}, stop ${:.4} ({:.2}% abajo)",
            if self.live { "REAL" } else { "PAPEL" },
            symbol,
            sig.breakout_high,
            self.cfg.breakout_hours,
            sig.close,
            notional,
            stop,
            dist / sig.close * dec!(100)
        );

        if !self.live {
            let entry = sig.close * (Decimal::ONE + PAPER_COST);
            self.paper_equity -= notional * PAPER_COST;
            let st = self.states.entry(symbol.to_string()).or_default();
            st.paper = Some(PaperPosition { size, entry });
            st.stop = Some(entry - dist);
            st.entry = Some(entry);
            st.initial_stop = Some(entry - dist);
            info!("{}", msg);
            let _ = self.notifier.send_message(&msg).await;
            self.notifier.publish_signal(SignalEvent::Entry {
                symbol: symbol.to_string(),
                price: entry,
                stop: entry - dist,
                paper: true,
                strategy: STRATEGY_TREND.into(),
                notional: Some(notional),
            });
            return Some(notional);
        }

        let asset = self.asset(symbol)?;
        let mid = match self.client.get_mid_price(symbol).await {
            Ok(m) => m,
            Err(e) => {
                error!("Sin precio para {}: {}", symbol, e);
                return None;
            }
        };
        let limit = mid * (Decimal::ONE + self.cfg.entry_slippage);
        if let Err(e) = self
            .client
            .place_order_ext(asset, HyperliquidOrderSide::Buy, size, limit, hypersdk::hypercore::types::TimeInForce::Ioc, false)
            .await
        {
            error!("❌ Entrada en {} falló: {}", symbol, e);
            return None;
        }

        // Leer la posición real y proteger con stop
        let (filled, entry) = match self.client.get_position(symbol).await {
            Ok(p) => p,
            Err(e) => {
                error!("No se pudo leer la posición de {} tras la entrada: {}", symbol, e);
                return Some(notional);
            }
        };
        if filled <= Decimal::ZERO {
            warn!("{}: la orden IOC no se ejecutó", symbol);
            return None;
        }
        let stop = entry - dist;
        match self.client.place_stop_loss(symbol, true, filled, stop).await {
            Ok(oid) => {
                let st = self.states.entry(symbol.to_string()).or_default();
                st.stop = Some(stop);
                st.stop_oid = oid;
                st.entry = Some(entry);
                st.initial_stop = Some(stop);
                info!("{}", msg);
                let _ = self.notifier.send_message(&msg).await;
                self.notifier.publish_signal(SignalEvent::Entry {
                    symbol: symbol.to_string(),
                    price: entry,
                    stop,
                    paper: false,
                    strategy: STRATEGY_TREND.into(),
                    notional: Some(filled * entry),
                });
            }
            Err(e) => {
                error!("❌ {}: stop no colocado tras la entrada ({}); cerrando", symbol, e);
                self.exit(symbol, "sin stop").await;
                return None;
            }
        }
        Some(filled * entry)
    }

    async fn exit(&mut self, symbol: &str, reason: &str) {
        let state = self.states.get(symbol).cloned().unwrap_or_default();
        if self.live {
            if let Some(oid) = state.stop_oid {
                if let Some(asset) = self.asset(symbol) {
                    let _ = self.client.cancel_order(asset, oid).await;
                }
            }
            let mid = self.client.get_mid_price(symbol).await.ok();
            match self.client.close_position(symbol, dec!(0.01)).await {
                Ok(()) => {
                    let msg = format!("🔻 SALIDA {}: {}", symbol, reason);
                    info!("{}", msg);
                    let _ = self.notifier.send_message(&msg).await;
                    if let (Some(entry), Some(exit)) = (state.entry, mid) {
                        self.notifier.publish_signal(SignalEvent::Exit {
                            symbol: symbol.to_string(),
                            entry,
                            exit,
                            initial_stop: state.initial_stop,
                            reason: reason.to_string(),
                            paper: false,
                            strategy: STRATEGY_TREND.into(),
                        });
                    }
                }
                Err(e) => error!("❌ No se pudo cerrar {}: {}", symbol, e),
            }
        } else if let Some(p) = state.paper.clone() {
            if let Ok(mid) = self.client.get_mid_price(symbol).await {
                self.close_paper(symbol, &p, mid, reason, state.initial_stop).await;
            }
        }
        if let Some(st) = self.states.get_mut(symbol) {
            *st = CoinState::default();
        }
    }

    /// Sin posición: limpia estado y stops huérfanos (p. ej. el stop ya se ejecutó)
    async fn clear_state(&mut self, symbol: &str) {
        let had_stop = self.states.get(symbol).map_or(false, |s| s.stop.is_some());
        if !had_stop {
            return;
        }
        if self.live {
            info!("ℹ️ {}: posición cerrada (stop ejecutado en el exchange)", symbol);
            if let Some(st) = self.states.get(symbol).cloned() {
                if let (Some(entry), Some(stop)) = (st.entry, st.stop) {
                    self.notifier.publish_signal(SignalEvent::Exit {
                        symbol: symbol.to_string(),
                        entry,
                        exit: stop,
                        initial_stop: st.initial_stop,
                        reason: "stop".to_string(),
                        paper: false,
                        strategy: STRATEGY_TREND.into(),
                    });
                }
            }
            if let Ok(stops) = self.client.get_stop_orders(symbol).await {
                if let Some(asset) = self.asset(symbol) {
                    for (oid, _) in stops {
                        let _ = self.client.cancel_order(asset, oid).await;
                    }
                }
            }
        }
        if let Some(st) = self.states.get_mut(symbol) {
            *st = CoinState::default();
        }
    }

    // ------------------------------------------------------------------
    // Papel
    // ------------------------------------------------------------------

    async fn check_paper_stops(&mut self) {
        let open: Vec<(String, PaperPosition, Decimal, Option<Decimal>)> = self
            .states
            .iter()
            .filter_map(|(s, st)| Some((s.clone(), st.paper.clone()?, st.stop?, st.initial_stop)))
            .collect();
        if open.is_empty() {
            return;
        }
        let mids = match self.client.get_all_mids().await {
            Ok(m) => m,
            Err(_) => return,
        };
        for (symbol, pos, stop, initial_stop) in open {
            if let Some(&mid) = mids.get(&symbol) {
                if mid <= stop {
                    self.close_paper(&symbol, &pos, mid, "stop", initial_stop).await;
                    if let Some(st) = self.states.get_mut(&symbol) {
                        *st = CoinState::default();
                    }
                }
            }
        }
    }

    async fn close_paper(
        &mut self,
        symbol: &str,
        pos: &PaperPosition,
        price: Decimal,
        reason: &str,
        initial_stop: Option<Decimal>,
    ) {
        let exit = price * (Decimal::ONE - PAPER_COST);
        let pnl = pos.size * (exit - pos.entry);
        self.paper_equity += pnl;
        let msg = format!(
            "📄 SALIDA PAPEL {} ({}): entrada ${:.4} salida ${:.4} P&L ${:.2} | equity ${:.2}",
            symbol, reason, pos.entry, exit, pnl, self.paper_equity
        );
        info!("{}", msg);
        let _ = self.notifier.send_message(&msg).await;
        self.notifier.publish_signal(SignalEvent::Exit {
            symbol: symbol.to_string(),
            entry: pos.entry,
            exit,
            initial_stop,
            reason: reason.to_string(),
            paper: true,
            strategy: STRATEGY_TREND.into(),
        });
    }

    // ------------------------------------------------------------------
    // Estrategia de rebote
    // ------------------------------------------------------------------

    /// El rebote opera con dinero real solo si el bot es real Y REBOUND_LIVE=true
    fn rebound_is_live(&self) -> bool {
        self.live && self.rebound_cfg.live
    }

    fn save_rebound(&self) {
        match serde_json::to_string_pretty(&self.rebound) {
            Ok(s) => {
                if let Err(e) = std::fs::write(&self.rebound_file, s) {
                    error!("❌ No se pudo guardar {:?}: {}", self.rebound_file, e);
                }
            }
            Err(e) => error!("❌ Error serializando posiciones de rebote: {}", e),
        }
    }

    /// Gestiona salidas y entradas del rebote. Devuelve el nocional total actualizado.
    async fn evaluate_rebound(
        &mut self,
        signals: &HashMap<String, Signal>,
        selected: &[String],
        trend_held: &[String],
        exchange_positions: &HashMap<String, (Decimal, Decimal)>,
        equity: Decimal,
        mut used_notional: Decimal,
    ) -> Decimal {
        let now = now_ms();

        // 1. Salidas
        for (symbol, pos) in self.rebound.clone() {
            // Real: si la posición ya no está en el exchange, saltó el stop
            if !pos.paper && !exchange_positions.contains_key(&symbol) {
                info!("ℹ️ {}: rebote cerrado por stop en el exchange", symbol);
                self.publish_rebound_exit(&symbol, &pos, pos.stop, "stop");
                self.rebound.remove(&symbol);
                self.save_rebound();
                continue;
            }
            let Some(sig) = signals.get(&symbol) else { continue };
            let held_h = now.saturating_sub(pos.opened_ms) / HOUR_MS;
            let reason = if sig.rsi > self.rebound_cfg.exit_rsi {
                Some(format!("rebote completado (RSI {:.0})", sig.rsi))
            } else if held_h >= self.rebound_cfg.max_hold_hours {
                Some(format!("{} h sin rebote suficiente", held_h))
            } else {
                None
            };
            if let Some(reason) = reason {
                used_notional -= pos.size * sig.close;
                self.rebound_exit(&symbol, &pos, sig.close, &reason).await;
            }
        }

        // 2. Entradas: las más sobrevendidas primero
        let mut candidates: Vec<(&String, &Signal)> = signals
            .iter()
            .filter(|(s, sig)| {
                sig.rsi < self.rebound_cfg.entry_rsi
                    && selected.contains(*s)
                    && !self.rebound.contains_key(*s)
                    && !trend_held.contains(*s)
                    && !(self.rebound_is_live() && exchange_positions.contains_key(*s))
            })
            .collect();
        candidates.sort_by(|a, b| a.1.rsi.cmp(&b.1.rsi));

        let paused = self.paused.load(Ordering::Relaxed);
        for (symbol, sig) in candidates {
            if paused {
                info!("⏸️ {}: sobreventa (RSI {:.0}) ignorada (bot en pausa)", symbol, sig.rsi);
                continue;
            }
            if self.rebound.len() >= self.rebound_cfg.max_positions {
                info!(
                    "⏸️ {}: sobreventa (RSI {:.0}) ignorada, máximo de {} posiciones de rebote",
                    symbol, sig.rsi, self.rebound_cfg.max_positions
                );
                continue;
            }
            let dist = self.rebound_cfg.stop_atr * sig.atr;
            if dist <= Decimal::ZERO || sig.close <= Decimal::ZERO {
                continue;
            }
            let stop_frac = dist / sig.close;
            let room = equity * self.cfg.max_total_leverage - used_notional;
            let Some(notional) = entry_notional(
                equity,
                self.rebound_cfg.risk_per_trade,
                self.cfg.max_risk_per_trade.max(self.rebound_cfg.risk_per_trade),
                stop_frac,
                room,
            ) else {
                info!(
                    "⏸️ {}: sobreventa (RSI {:.0}) sin tamaño suficiente (stop a {:.2}%, cupo ${:.2})",
                    symbol,
                    sig.rsi,
                    stop_frac * dec!(100),
                    room
                );
                continue;
            };
            if let Some(n) = self.rebound_enter(symbol, sig, notional, dist).await {
                used_notional += n;
            }
        }
        used_notional
    }

    async fn rebound_enter(&mut self, symbol: &str, sig: &Signal, notional: Decimal, dist: Decimal) -> Option<Decimal> {
        let live = self.rebound_is_live();
        let (size, entry, stop_oid) = if live {
            let asset = self.asset(symbol)?;
            let mid = self.client.get_mid_price(symbol).await.ok()?;
            let limit = mid * (Decimal::ONE + self.cfg.entry_slippage);
            if let Err(e) = self
                .client
                .place_order_ext(asset, HyperliquidOrderSide::Buy, notional / sig.close, limit, hypersdk::hypercore::types::TimeInForce::Ioc, false)
                .await
            {
                error!("❌ Rebote: entrada en {} falló: {}", symbol, e);
                return None;
            }
            let (filled, entry) = match self.client.get_position(symbol).await {
                Ok(p) if p.0 > Decimal::ZERO => p,
                _ => {
                    warn!("Rebote {}: la orden IOC no se ejecutó", symbol);
                    return None;
                }
            };
            match self.client.place_stop_loss(symbol, true, filled, entry - dist).await {
                Ok(oid) => (filled, entry, oid),
                Err(e) => {
                    error!("❌ Rebote {}: stop no colocado ({}); cerrando", symbol, e);
                    let _ = self.client.close_position(symbol, dec!(0.01)).await;
                    return None;
                }
            }
        } else {
            let entry = sig.close * (Decimal::ONE + PAPER_COST);
            self.paper_equity -= notional * PAPER_COST;
            (notional / entry, entry, None)
        };

        let pos = ReboundPosition {
            size,
            entry,
            stop: entry - dist,
            opened_ms: now_ms(),
            paper: !live,
            stop_oid,
        };
        let msg = format!(
            "🔄 ENTRADA REBOTE {} {}: RSI {:.0} a ${:.4}, nocional ${:.2}, stop ${:.4} ({:.2}% abajo), salida con RSI > {} o en {} h",
            if live { "REAL" } else { "PAPEL" },
            symbol,
            sig.rsi,
            entry,
            size * entry,
            pos.stop,
            dist / sig.close * dec!(100),
            self.rebound_cfg.exit_rsi,
            self.rebound_cfg.max_hold_hours
        );
        info!("{}", msg);
        let _ = self.notifier.send_message(&msg).await;
        self.notifier.publish_signal(SignalEvent::Entry {
            symbol: symbol.to_string(),
            price: entry,
            stop: pos.stop,
            paper: !live,
            strategy: STRATEGY_REBOUND.into(),
            notional: Some(size * entry),
        });
        self.rebound.insert(symbol.to_string(), pos);
        self.save_rebound();
        Some(size * entry)
    }

    async fn rebound_exit(&mut self, symbol: &str, pos: &ReboundPosition, price: Decimal, reason: &str) {
        let exit_px = if pos.paper {
            let exit_px = price * (Decimal::ONE - PAPER_COST);
            self.paper_equity += pos.size * (exit_px - pos.entry);
            exit_px
        } else {
            if let (Some(oid), Some(asset)) = (pos.stop_oid, self.asset(symbol)) {
                let _ = self.client.cancel_order(asset, oid).await;
            }
            let mid = self.client.get_mid_price(symbol).await.unwrap_or(price);
            if let Err(e) = self.client.close_position(symbol, dec!(0.01)).await {
                error!("❌ Rebote: no se pudo cerrar {}: {}", symbol, e);
                return;
            }
            mid
        };
        let pnl = pos.size * (exit_px - pos.entry);
        let msg = format!(
            "🔄 SALIDA REBOTE {} {} ({}): entrada ${:.4} salida ${:.4} P&L ${:.2}",
            if pos.paper { "PAPEL" } else { "REAL" },
            symbol,
            reason,
            pos.entry,
            exit_px,
            pnl
        );
        info!("{}", msg);
        let _ = self.notifier.send_message(&msg).await;
        self.publish_rebound_exit(symbol, pos, exit_px, reason);
        self.rebound.remove(symbol);
        self.save_rebound();
    }

    fn publish_rebound_exit(&self, symbol: &str, pos: &ReboundPosition, exit: Decimal, reason: &str) {
        self.notifier.publish_signal(SignalEvent::Exit {
            symbol: symbol.to_string(),
            entry: pos.entry,
            exit,
            initial_stop: Some(pos.stop),
            reason: reason.to_string(),
            paper: pos.paper,
            strategy: STRATEGY_REBOUND.into(),
        });
    }

    /// Stops de las posiciones de rebote en papel (cada tick)
    async fn check_rebound_paper_stops(&mut self) {
        let open: Vec<(String, ReboundPosition)> = self
            .rebound
            .iter()
            .filter(|(_, p)| p.paper)
            .map(|(s, p)| (s.clone(), p.clone()))
            .collect();
        if open.is_empty() {
            return;
        }
        let Ok(mids) = self.client.get_all_mids().await else { return };
        for (symbol, pos) in open {
            if let Some(&mid) = mids.get(&symbol) {
                if mid <= pos.stop {
                    self.rebound_exit(&symbol, &pos, mid, "stop").await;
                }
            }
        }
    }

    // ------------------------------------------------------------------
    // Datos
    // ------------------------------------------------------------------

    /// Equity y posiciones largas/cortas (coin -> (tamaño, entrada))
    async fn account(&self) -> anyhow::Result<(Decimal, HashMap<String, (Decimal, Decimal)>)> {
        if self.live {
            return self.client.get_account_snapshot().await;
        }
        let mut positions = HashMap::new();
        let mut unrealized = Decimal::ZERO;
        let mids = self.client.get_all_mids().await.unwrap_or_default();
        for (symbol, st) in &self.states {
            if let Some(p) = &st.paper {
                positions.insert(symbol.clone(), (p.size, p.entry));
                if let Some(mid) = mids.get(symbol) {
                    unrealized += p.size * (*mid - p.entry);
                }
            }
        }
        for (symbol, rb) in &self.rebound {
            if rb.paper {
                if let Some(mid) = mids.get(symbol) {
                    unrealized += rb.size * (*mid - rb.entry);
                }
            }
        }
        Ok((self.paper_equity + unrealized, positions))
    }

    async fn signal(&self, symbol: &str) -> anyhow::Result<Option<Signal>> {
        let candles = self.fetch_closed_candles(symbol).await?;
        Ok(compute_signal(&candles, &self.cfg))
    }

    async fn fetch_closed_candles(&self, symbol: &str) -> anyhow::Result<Vec<Candle>> {
        let now = now_ms();
        let hours = (self.cfg.breakout_hours + self.cfg.atr_period * 4 + 10) as u64;
        let body = serde_json::json!({
            "type": "candleSnapshot",
            "req": { "coin": symbol, "interval": "1h", "startTime": now - hours * HOUR_MS, "endTime": now }
        });
        let raw: Vec<RawCandle> = self.http.post(self.info_url).json(&body).send().await?.json().await?;
        let mut out = Vec::with_capacity(raw.len());
        for c in raw {
            // Solo velas cerradas
            if c.close_time > now {
                continue;
            }
            out.push(Candle {
                t: c.t,
                high: c.h.parse()?,
                low: c.l.parse()?,
                close: c.c.parse()?,
            });
        }
        Ok(out)
    }

    /// Funding medio por hora de las últimas 24 h (API pública de Hyperliquid)
    async fn avg_funding_24h(&self, symbol: &str) -> anyhow::Result<Decimal> {
        #[derive(Deserialize)]
        struct F {
            #[serde(rename = "fundingRate")]
            funding_rate: String,
        }
        let body = serde_json::json!({
            "type": "fundingHistory",
            "coin": symbol,
            "startTime": now_ms().saturating_sub(24 * HOUR_MS),
        });
        let rows: Vec<F> = self.http.post(self.info_url).json(&body).send().await?.json().await?;
        let rates: Vec<Decimal> = rows.iter().filter_map(|r| r.funding_rate.parse().ok()).collect();
        if rates.is_empty() {
            anyhow::bail!("sin datos");
        }
        Ok(rates.iter().sum::<Decimal>() / Decimal::from(rates.len() as u64))
    }

    fn asset(&self, symbol: &str) -> Option<u32> {
        self.client.get_market_info(symbol).map(|m| m.index as u32)
    }
}

/// Indicadores sobre velas cerradas (la última es la más reciente)
fn compute_signal(candles: &[Candle], cfg: &TrendConfig) -> Option<Signal> {
    let n = candles.len();
    if n < cfg.breakout_hours + 2 || n < cfg.atr_period + 2 {
        return None;
    }

    // ATR con suavizado de Wilder (igual que el backtest)
    let period = Decimal::from(cfg.atr_period as u64);
    let mut atr: Option<Decimal> = None;
    let mut prev_close: Option<Decimal> = None;
    for c in candles {
        let tr = match prev_close {
            None => c.high - c.low,
            Some(pc) => (c.high - c.low).max((c.high - pc).abs()).max((c.low - pc).abs()),
        };
        atr = Some(match atr {
            None => tr,
            Some(a) => a + (tr - a) / period,
        });
        prev_close = Some(c.close);
    }

    // RSI(14) de Wilder (igual que tools/backtest/dips.py)
    let mut avg_gain: Option<Decimal> = None;
    let mut avg_loss: Option<Decimal> = None;
    let rsi_period = dec!(14);
    for w in candles.windows(2) {
        let ch = w[1].close - w[0].close;
        let (g, l) = if ch > Decimal::ZERO { (ch, Decimal::ZERO) } else { (Decimal::ZERO, -ch) };
        avg_gain = Some(avg_gain.map_or(g, |a| a + (g - a) / rsi_period));
        avg_loss = Some(avg_loss.map_or(l, |a| a + (l - a) / rsi_period));
    }
    let rsi = match (avg_gain, avg_loss) {
        (Some(g), Some(l)) if l > Decimal::ZERO => dec!(100) - dec!(100) / (Decimal::ONE + g / l),
        (Some(_), Some(_)) => dec!(100),
        _ => dec!(50),
    };

    let last = &candles[n - 1];
    let prior = &candles[n - 1 - cfg.breakout_hours..n - 1];
    let exit_prior = &candles[n - 1 - cfg.exit_hours.min(n - 1)..n - 1];
    let breakout_high = prior.iter().map(|c| c.high).max()?;
    let exit_low = exit_prior.iter().map(|c| c.low).min()?;

    Some(Signal {
        candle_t: last.t,
        close: last.close,
        atr: atr?,
        breakout_high,
        exit_low,
        rsi,
    })
}

fn now_ms() -> u64 {
    chrono::Utc::now().timestamp_millis().max(0) as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::prelude::ToPrimitive;

    fn candle(t: u64, h: f64, l: f64, c: f64) -> Candle {
        use rust_decimal::prelude::FromPrimitive;
        Candle {
            t,
            high: Decimal::from_f64(h).unwrap(),
            low: Decimal::from_f64(l).unwrap(),
            close: Decimal::from_f64(c).unwrap(),
        }
    }

    #[test]
    fn breakout_detected_only_above_prior_high() {
        let cfg = TrendConfig { breakout_hours: 10, exit_hours: 5, atr_period: 3, ..Default::default() };
        let mut cs: Vec<Candle> = (0..20).map(|i| candle(i, 101.0, 99.0, 100.0)).collect();
        cs.push(candle(20, 103.0, 100.0, 102.5));
        let s = compute_signal(&cs, &cfg).unwrap();
        assert!(s.is_breakout());
        assert_eq!(s.breakout_high, dec!(101));
        assert_eq!(s.exit_low, dec!(99));

        cs.pop();
        cs.push(candle(20, 101.0, 99.5, 100.5));
        assert!(!compute_signal(&cs, &cfg).unwrap().is_breakout());
    }

    #[test]
    fn small_positions_are_raised_to_minimum_within_max_risk() {
        let eq = dec!(50);
        // BTC: stop 0.83% -> por riesgo $30, sin ajuste
        assert_eq!(
            entry_notional(eq, dec!(0.005), dec!(0.0125), dec!(0.0083), dec!(150)).unwrap().round_dp(2),
            dec!(30.12)
        );
        // NEAR: stop 6.13% -> por riesgo $4.08; con $10 arriesga 1.23% <= 1.25% -> $10
        assert_eq!(entry_notional(eq, dec!(0.005), dec!(0.0125), dec!(0.0613), dec!(150)), Some(dec!(10)));
        // Stop 7%: $10 arriesgaría 1.4% > 1.25% -> no se opera
        assert_eq!(entry_notional(eq, dec!(0.005), dec!(0.0125), dec!(0.07), dec!(150)), None);
        // Sin cupo de apalancamiento para $10 -> no se opera
        assert_eq!(entry_notional(eq, dec!(0.005), dec!(0.0125), dec!(0.0613), dec!(8)), None);
        // max_risk == risk: comportamiento anterior (se omite)
        assert_eq!(entry_notional(eq, dec!(0.005), dec!(0.005), dec!(0.0613), dec!(150)), None);
    }

    #[test]
    fn rsi_is_low_after_steady_drop_and_high_after_rise() {
        let cfg = TrendConfig { breakout_hours: 24, exit_hours: 6, atr_period: 3, ..Default::default() };
        let down: Vec<Candle> = (0..60)
            .map(|i| {
                let p = 100.0 - i as f64 * 0.5;
                candle(i, p + 0.2, p - 0.2, p)
            })
            .collect();
        assert!(compute_signal(&down, &cfg).unwrap().rsi < dec!(20));
        let up: Vec<Candle> = (0..60)
            .map(|i| {
                let p = 100.0 + i as f64 * 0.5;
                candle(i, p + 0.2, p - 0.2, p)
            })
            .collect();
        assert!(compute_signal(&up, &cfg).unwrap().rsi > dec!(80));
    }

    #[test]
    fn overrides_are_applied_with_safety_limits_and_never_enable_live() {
        let t = TrendConfig::default();
        let r = ReboundConfig::default();
        let o = StrategyOverrides {
            trend_risk_per_trade: Some(0.5), // absurdo: se limita a 2%
            trend_max_positions: Some(3),
            rebound_enabled: Some(false),
            rebound_entry_rsi: Some(1.0), // se limita a 5
            ..Default::default()
        };
        let (t2, r2) = o.apply(&t, &r);
        assert_eq!(t2.risk_per_trade, dec!(0.02));
        assert_eq!(t2.max_positions, 3);
        assert!(t2.funding_filter); // activo por defecto
        assert!(!r2.enabled);
        assert_eq!(r2.entry_rsi, dec!(5));
        assert!(!r2.live);
        // Sin ajustes: igual que antes
        let (t3, _) = StrategyOverrides::default().apply(&t, &r);
        assert_eq!(t3.risk_per_trade, t.risk_per_trade);
    }

    #[test]
    fn rebound_config_is_paper_by_default() {
        let c = ReboundConfig::default();
        assert!(c.enabled && !c.live);
        assert_eq!(c.entry_rsi, dec!(20));
        assert_eq!(c.max_hold_hours, 48);
    }

    #[test]
    fn needs_enough_history() {
        let cfg = TrendConfig::default();
        let cs: Vec<Candle> = (0..50).map(|i| candle(i, 1.0, 1.0, 1.0)).collect();
        assert!(compute_signal(&cs, &cfg).is_none());
    }

    #[test]
    fn atr_is_positive_for_ranging_market() {
        let cfg = TrendConfig { breakout_hours: 5, exit_hours: 3, atr_period: 3, ..Default::default() };
        let cs: Vec<Candle> = (0..10).map(|i| candle(i, 102.0, 98.0, 100.0)).collect();
        let s = compute_signal(&cs, &cfg).unwrap();
        assert_eq!(s.atr, dec!(4));
        assert!(s.atr.to_f64().unwrap() > 0.0);
    }
}
