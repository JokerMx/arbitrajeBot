//! Implementación de la estrategia Avellaneda-Stoikov

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use rust_decimal::prelude::FromPrimitive;
use rust_decimal::MathematicalOps;
use tracing::debug;
use std::collections::VecDeque;

use crate::types::{L2Book, StrategyConfig};

/// Calculadora de volatilidad
#[derive(Clone)]  // AÑADIDO Clone
pub struct VolatilityCalculator {
    prices: VecDeque<Decimal>,
    lookback_minutes: u64,
}

impl VolatilityCalculator {
    pub fn new(lookback_minutes: u64) -> Self {
        Self {
            prices: VecDeque::with_capacity(lookback_minutes as usize),
            lookback_minutes,
        }
    }

    pub fn add_price(&mut self, price: Decimal) {
        self.prices.push_back(price);
        let max_len = self.lookback_minutes as usize;
        while self.prices.len() > max_len {
            self.prices.pop_front();
        }
    }

    pub fn calculate(&self) -> Decimal {
        if self.prices.len() < 2 {
            return dec!(0.0001);
        }

        let mut returns = Vec::with_capacity(self.prices.len() - 1);
        for i in 1..self.prices.len() {
            let prev = self.prices[i-1];
            let curr = self.prices[i];
            if prev > Decimal::ZERO && curr > Decimal::ZERO {
                let ret = (curr - prev) / prev;
                returns.push(ret);
            }
        }

        if returns.is_empty() {
            return dec!(0.0001);
        }

        let sum: Decimal = returns.iter().sum();
        let mean = sum / Decimal::from(returns.len());

        let variance_sum: Decimal = returns.iter()
            .map(|r| {
                let diff = *r - mean;
                diff.powi(2)
            })
            .sum();
        let variance = variance_sum / Decimal::from(returns.len());

        let volatility = (variance * Decimal::from(525600)).sqrt().unwrap_or(dec!(0.0001));
        
        if volatility > dec!(0.05) {
            dec!(0.05)
        } else if volatility < dec!(0.0001) {
            dec!(0.0001)
        } else {
            volatility
        }
    }
}

/// Precios calculados
#[derive(Debug, Clone)]  // AÑADIDO Clone
pub struct QuotedPrices {
    pub bid: Decimal,
    pub ask: Decimal,
    pub reservation: Decimal,
    pub spread: Decimal,
}

/// Estrategia Avellaneda-Stoikov
#[derive(Clone)]  // AÑADIDO Clone
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
            self.last_mid_price = mid_price;
            self.volatility_calc.add_price(mid_price);
            debug!("Mid price updated para {}: ${:.2}", book.coin, mid_price);
        }
    }

    pub fn calculate_quotes(
        &self,
        inventory: Decimal,
        max_inventory: Decimal,
    ) -> QuotedPrices {
        let mid = self.last_mid_price;
        
        if mid == Decimal::ZERO {
            return QuotedPrices {
                bid: dec!(50000),
                ask: dec!(50100),
                reservation: dec!(50050),
                spread: dec!(100),
            };
        }

        let volatility = self.volatility_calc.calculate();
        
        let gamma = Decimal::from_f64(self.config.risk_aversion.min(1.0).max(0.01)).unwrap_or(dec!(0.1));
        let _k = Decimal::from_f64(self.config.market_impact.min(0.1).max(0.001)).unwrap_or(dec!(0.01));
        let inventory_skew_strength = Decimal::from_f64(self.config.inventory_skew_strength.min(1.0).max(0.0)).unwrap_or(dec!(0.5));
        
        let inventory_ratio = if max_inventory > Decimal::ZERO {
            (inventory / max_inventory).abs().min(dec!(1.0))
        } else {
            Decimal::ZERO
        };
        
        let max_skew = dec!(0.01);
        let inventory_skew = -gamma * volatility * inventory_ratio * inventory_skew_strength;
        let inventory_skew = inventory_skew.max(-max_skew).min(max_skew);
        
        let reservation_price = mid * (Decimal::ONE + inventory_skew);
        
        let spread_pct = (gamma * volatility + dec!(0.0005)).min(dec!(0.02));
        let spread = mid * spread_pct;
        
        let tick_size = match self.config.base_spread_ticks {
            _ if mid > dec!(10000) => dec!(1.0),
            _ if mid > dec!(1000) => dec!(0.1),
            _ => dec!(0.01),
        };
        
        let min_spread = tick_size * Decimal::from(self.config.base_spread_ticks.max(1));
        let spread = spread.max(min_spread);
        
        let half_spread = spread / dec!(2);
        let bid_price = (reservation_price - half_spread).max(mid * dec!(0.9));
        let ask_price = (reservation_price + half_spread).min(mid * dec!(1.1));
        
        let bid_price = (bid_price / tick_size).floor() * tick_size;
        let ask_price = (ask_price / tick_size).ceil() * tick_size;
        
        debug!("Quotes para ${:.2}: bid=${:.2}, ask=${:.2}, spread={:.2}%", 
               mid, bid_price, ask_price, spread_pct * dec!(100));
        
        QuotedPrices {
            bid: bid_price,
            ask: ask_price,
            reservation: reservation_price,
            spread,
        }
    }

    pub fn should_refresh(&self, last_refresh: chrono::DateTime<chrono::Utc>) -> bool {
        let elapsed = (chrono::Utc::now() - last_refresh).num_milliseconds() as u64;
        elapsed >= self.config.order_refresh_time_ms
    }
}