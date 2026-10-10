//! Implementación de la estrategia Avellaneda-Stoikov (simplificada)
//!
//! Unidades: todos los porcentajes son fracciones (0.001 = 0.1%).
//! La volatilidad se mide por muestra (no anualizada) y se escala al
//! horizonte de una cotización, para que el spread sea coherente con
//! la frecuencia real de muestreo del bot.

use rust_decimal::prelude::FromPrimitive;
use rust_decimal::Decimal;
use rust_decimal::MathematicalOps;
use rust_decimal_macros::dec;
use std::collections::VecDeque;
use tracing::debug;

use crate::types::{L2Book, StrategyConfig};

/// Fee maker de Hyperliquid (tier base). Un ciclo compra+venta paga 2 veces.
pub const MAKER_FEE: Decimal = dec!(0.00015);
/// Ganancia mínima por ciclo completo, por encima de las fees
pub const MIN_EDGE: Decimal = dec!(0.0004);
/// Segundos entre muestras de precio (intervalo del loop de bot.rs)
pub const SAMPLE_INTERVAL_SECS: u64 = 10;
/// Número de muestras que representa el horizonte de una cotización (~1 min)
const HORIZON_SAMPLES: u32 = 6;
/// Spread máximo total
const MAX_SPREAD: Decimal = dec!(0.02);
/// Medio spread mínimo en múltiplos de la volatilidad del horizonte.
/// Backtest (velas 1m de BTC/ETH/SOL): con 1.25σ el bot perdía 4-8% en 3.6 días por
/// selección adversa; con 5σ + filtro de tendencia + salida por tiempo quedó cerca de 0.
const MIN_VOL_MULT: Decimal = dec!(5);
/// Ventana del filtro de tendencia (muestras de 10 s = 5 min)
const TREND_SAMPLES: usize = 30;
/// Umbral del filtro de tendencia en σ de la ventana
const TREND_THRESHOLD_SIGMAS: Decimal = dec!(2);

/// Calculadora de volatilidad (desviación estándar de retornos por muestra)
#[derive(Clone)]
pub struct VolatilityCalculator {
    prices: VecDeque<Decimal>,
    max_samples: usize,
}

impl VolatilityCalculator {
    pub fn new(lookback_minutes: u64) -> Self {
        let max_samples = ((lookback_minutes.max(1) * 60) / SAMPLE_INTERVAL_SECS).max(2) as usize;
        Self {
            prices: VecDeque::with_capacity(max_samples),
            max_samples,
        }
    }

    pub fn add_price(&mut self, price: Decimal) {
        self.prices.push_back(price);
        while self.prices.len() > self.max_samples {
            self.prices.pop_front();
        }
    }

    pub fn samples(&self) -> usize {
        self.prices.len()
    }

    /// Retorno entre la muestra de hace `n` y la última
    pub fn return_over(&self, n: usize) -> Option<Decimal> {
        let len = self.prices.len();
        if len <= n {
            return None;
        }
        let old = self.prices[len - 1 - n];
        let last = self.prices[len - 1];
        if old <= Decimal::ZERO {
            return None;
        }
        Some((last - old) / old)
    }

    /// Volatilidad por muestra. `None` si aún no hay datos suficientes.
    pub fn calculate(&self) -> Option<Decimal> {
        if self.prices.len() < 3 {
            return None;
        }

        let returns: Vec<Decimal> = self
            .prices
            .iter()
            .zip(self.prices.iter().skip(1))
            .filter(|(prev, curr)| **prev > Decimal::ZERO && **curr > Decimal::ZERO)
            .map(|(prev, curr)| (*curr - *prev) / *prev)
            .collect();

        if returns.len() < 2 {
            return None;
        }

        let n = Decimal::from(returns.len());
        let mean = returns.iter().sum::<Decimal>() / n;
        let variance = returns.iter().map(|r| (*r - mean).powi(2)).sum::<Decimal>() / n;
        variance.sqrt()
    }
}

/// Precios calculados
#[derive(Debug, Clone)]
pub struct QuotedPrices {
    pub bid: Decimal,
    pub ask: Decimal,
    pub reservation: Decimal,
    /// Spread total como fracción del mid
    pub spread_pct: Decimal,
}

/// Estrategia Avellaneda-Stoikov
#[derive(Clone)]
pub struct AvellanedaStoikov {
    config: StrategyConfig,
    volatility_calc: VolatilityCalculator,
    last_mid_price: Decimal,
}

impl AvellanedaStoikov {
    pub fn new(config: StrategyConfig) -> Self {
        Self {
            volatility_calc: VolatilityCalculator::new(config.volatility_lookback_minutes),
            config,
            last_mid_price: Decimal::ZERO,
        }
    }

    pub fn update(&mut self, book: &L2Book) {
        if let (Some(best_bid), Some(best_ask)) = (book.bids.first(), book.asks.first()) {
            let mid_price = (best_bid.px + best_ask.px) / dec!(2);
            self.update_price(mid_price);
            debug!("Mid price updated para {}: ${:.2}", book.coin, mid_price);
        }
    }

    pub fn update_price(&mut self, mid_price: Decimal) {
        if mid_price > Decimal::ZERO {
            self.last_mid_price = mid_price;
            self.volatility_calc.add_price(mid_price);
        }
    }

    /// `true` cuando hay suficientes muestras para estimar volatilidad
    pub fn is_warmed_up(&self) -> bool {
        self.volatility_calc.calculate().is_some()
    }

    /// Spread mínimo total para que un ciclo maker-maker sea rentable
    pub fn min_spread_pct() -> Decimal {
        MAKER_FEE * dec!(2) + MIN_EDGE
    }

    /// Calcula bid/ask.
    /// `inventory` es la posición con signo (en unidades del activo) y
    /// `max_inventory` la posición máxima permitida (en unidades).
    pub fn calculate_quotes(&self, inventory: Decimal, max_inventory: Decimal) -> Option<QuotedPrices> {
        let mid = self.last_mid_price;
        if mid <= Decimal::ZERO {
            return None;
        }
        let sigma = self.volatility_calc.calculate()?;

        let gamma = Decimal::from_f64(self.config.risk_aversion.clamp(0.01, 1.0)).unwrap_or(dec!(0.1));
        let skew_strength =
            Decimal::from_f64(self.config.inventory_skew_strength.clamp(0.0, 1.0)).unwrap_or(dec!(0.5));

        // Inventario normalizado CON signo: +1 = máximo largo, -1 = máximo corto
        let q = if max_inventory > Decimal::ZERO {
            (inventory / max_inventory).max(-Decimal::ONE).min(Decimal::ONE)
        } else {
            Decimal::ZERO
        };

        // Volatilidad esperada durante el horizonte de la cotización
        let horizon_sigma = sigma * Decimal::from(HORIZON_SAMPLES).sqrt().unwrap_or(dec!(2.45));

        // Medio spread: al menos cubre fees + edge, y crece con la volatilidad
        let vol_mult = (dec!(0.5) + gamma * dec!(5)).max(MIN_VOL_MULT);
        let half_spread = (horizon_sigma * vol_mult)
            .max(Self::min_spread_pct() / dec!(2))
            .min(MAX_SPREAD / dec!(2));

        // Precio de reserva: si estamos largos baja (vendemos antes), si cortos sube
        let reservation = mid * (Decimal::ONE - q * skew_strength * half_spread * dec!(2));

        // Nunca cotizar del lado equivocado del mid (ALO lo rechazaría)
        let min_offset = MAKER_FEE;
        let bid = (reservation * (Decimal::ONE - half_spread)).min(mid * (Decimal::ONE - min_offset));
        let ask = (reservation * (Decimal::ONE + half_spread)).max(mid * (Decimal::ONE + min_offset));

        debug!(
            "Quotes mid=${:.4} σ={:.6} q={:.3}: bid=${:.4} ask=${:.4} spread={:.3}%",
            mid,
            sigma,
            q,
            bid,
            ask,
            (ask - bid) / mid * dec!(100)
        );

        Some(QuotedPrices {
            bid,
            ask,
            reservation,
            spread_pct: (ask - bid) / mid,
        })
    }

    /// Filtro de tendencia: `(permitir_compra, permitir_venta)` para ABRIR posición.
    /// No se compra durante una caída fuerte ni se vende durante una subida fuerte,
    /// que es cuando las órdenes pasivas se ejecutan justo antes de perder.
    pub fn trend_filter(&self) -> (bool, bool) {
        let (Some(sigma), Some(ret)) = (
            self.volatility_calc.calculate(),
            self.volatility_calc.return_over(TREND_SAMPLES),
        ) else {
            return (true, true);
        };
        let window_sigma = sigma * Decimal::from(TREND_SAMPLES as u32).sqrt().unwrap_or(dec!(5.48));
        let threshold = window_sigma * TREND_THRESHOLD_SIGMAS;
        let allow_buy = ret >= -threshold;
        let allow_sell = ret <= threshold;
        if !allow_buy || !allow_sell {
            debug!("📈 Filtro de tendencia: retorno 5m {:.4}% (umbral {:.4}%)", ret * dec!(100), threshold * dec!(100));
        }
        (allow_buy, allow_sell)
    }

    pub fn should_refresh(&self, last_refresh: chrono::DateTime<chrono::Utc>) -> bool {
        let elapsed = (chrono::Utc::now() - last_refresh).num_milliseconds().max(0) as u64;
        elapsed >= self.config.order_refresh_time_ms
    }
}
