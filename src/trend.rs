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
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{error, info, warn};

use crate::hyperliquid::client::STOP_SLIPPAGE;
use crate::hyperliquid::{HyperliquidClient, HyperliquidOrderSide};
use crate::notifier::TelegramNotifier;
use crate::risk::RiskManager;
use crate::signals::SignalEvent;

const HOUR_MS: u64 = 3_600_000;
/// Espera tras el cierre de la vela antes de evaluar (la API tarda en publicarla)
const EVAL_DELAY_MS: u64 = 20_000;
/// Fee taker + slippage estimado (para el P&L en papel)
const PAPER_COST: Decimal = dec!(0.00095);
/// Mínimo de Hyperliquid por orden
const MIN_ORDER_USD: Decimal = dec!(10);

#[derive(Debug, Clone)]
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
    /// Nocional total máximo / equity
    pub max_total_leverage: Decimal,
    pub max_positions: usize,
    /// Precio límite de la entrada IOC sobre el mid
    pub entry_slippage: Decimal,
}

impl Default for TrendConfig {
    fn default() -> Self {
        Self {
            breakout_hours: 168,
            exit_hours: 24,
            atr_period: 14,
            stop_atr: dec!(3),
            // Backtest: 0.5% -> DD máx ~7%; 1% -> DD ~20%; 2% -> DD ~44% (no usar)
            risk_per_trade: dec!(0.01),
            max_total_leverage: dec!(3),
            max_positions: 4,
            entry_slippage: dec!(0.005),
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
            max_total_leverage: var("TREND_MAX_LEVERAGE").unwrap_or(d.max_total_leverage),
            max_positions: var("TREND_MAX_POSITIONS").unwrap_or(d.max_positions),
            entry_slippage: d.entry_slippage,
        };
        // Límites de seguridad: el backtest mostró caídas de ~44% con 2% de riesgo
        Self {
            risk_per_trade: cfg.risk_per_trade.max(dec!(0.001)).min(dec!(0.02)),
            max_total_leverage: cfg.max_total_leverage.max(Decimal::ONE).min(dec!(5)),
            max_positions: cfg.max_positions.clamp(1, 10),
            breakout_hours: cfg.breakout_hours.clamp(24, 1000),
            exit_hours: cfg.exit_hours.clamp(6, 500),
            ..cfg
        }
    }
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
    symbols: Vec<String>,
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
    ) -> Self {
        let states = symbols.iter().map(|s| (s.clone(), CoinState::default())).collect();
        Self {
            cfg,
            symbols,
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
        }
    }

    pub async fn start(&mut self) -> anyhow::Result<()> {
        info!(
            "📈 Estrategia TENDENCIA ({}) en {:?}: ruptura {}h, salida {}h, stop {} ATR, riesgo {}%/op, máx {} posiciones, máx {}x",
            if self.live { "REAL" } else { "PAPEL" },
            self.symbols,
            self.cfg.breakout_hours,
            self.cfg.exit_hours,
            self.cfg.stop_atr,
            self.cfg.risk_per_trade * dec!(100),
            self.cfg.max_positions,
            self.cfg.max_total_leverage
        );

        if self.live {
            // Recuperar stops existentes en el exchange (reinicio del bot)
            let (_, positions) = self.client.get_account_snapshot().await?;
            for symbol in self.symbols.clone() {
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
        let now = now_ms();
        let hour = now / HOUR_MS;
        if hour != self.last_eval_hour && now % HOUR_MS >= EVAL_DELAY_MS {
            self.last_eval_hour = hour;
            self.evaluate_all().await;
        }
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

        for symbol in self.symbols.clone() {
            let signal = match self.signal(&symbol).await {
                Ok(Some(s)) => s,
                Ok(None) => continue,
                Err(e) => {
                    warn!("Sin velas para {}: {}", symbol, e);
                    continue;
                }
            };

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
                    if signal.is_breakout() {
                        candidates.push((symbol.clone(), signal));
                    }
                }
            }
        }

        // 4. Entradas, las rupturas más fuertes primero
        candidates.sort_by(|a, b| b.1.strength().cmp(&a.1.strength()));
        for (symbol, signal) in candidates {
            if open_count >= self.cfg.max_positions {
                info!("⏸️ {}: ruptura ignorada, máximo de {} posiciones", symbol, self.cfg.max_positions);
                continue;
            }
            if let Some(notional) = self.enter(&symbol, &signal, equity, used_notional).await {
                open_count += 1;
                used_notional += notional;
            }
        }

        info!(
            "📊 Tendencia: equity ${:.2}, {} posiciones, nocional ${:.2} ({:.2}x)",
            equity,
            open_count,
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
        let by_risk = equity * self.cfg.risk_per_trade / (dist / sig.close);
        let room = equity * self.cfg.max_total_leverage - used;
        let notional = by_risk.min(room);
        if notional < MIN_ORDER_USD {
            info!(
                "⏸️ {}: ruptura sin tamaño suficiente (${:.2} < $10; riesgo ${:.2}, cupo ${:.2})",
                symbol, notional, by_risk, room
            );
            return None;
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
        });
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
