//! Ejecutor de órdenes reales usando hypersdk (EIP-712)

use anyhow::{Result, anyhow};
use rust_decimal::Decimal;
use tracing::info; // <-- AÑADIR ESTA LÍNEA
use uuid::Uuid;
use crate::types::OrderRequest;
use crate::hyperliquid::{HyperliquidClient, HyperliquidOrderSide};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ExchangeType {
    Simulated,
    HyperliquidMainnet,
}

pub struct OrderExecutor {
    exchange: ExchangeType,
    client: Option<HyperliquidClient>,
}

impl OrderExecutor {
    pub async fn new(private_key: &str, exchange: ExchangeType) -> Result<Self> {
        match exchange {
            ExchangeType::Simulated => {
                info!("🎮 Usando modo simulado");
                Ok(Self { exchange, client: None })
            }
            ExchangeType::HyperliquidMainnet => {
                info!("🌐 Conectando a Hyperliquid MAINNET con hypersdk...");
                let client = HyperliquidClient::new(private_key, true).await?;
                info!("✅ Cliente Hyperliquid inicializado (EIP-712)");
                Ok(Self { exchange, client: Some(client) })
            }
        }
    }

    pub async fn place_order(&self, order: OrderRequest) -> Result<String> {
        match self.exchange {
            ExchangeType::Simulated => {
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                let order_id = format!("sim_{}", Uuid::new_v4());
                info!(
                    "💰 [SIMULADO] Orden colocada: {:?} {} @ ${:.2} - ID: {}",
                    order.side,
                    order.quantity,
                    order.price.unwrap_or(Decimal::ZERO),
                    order_id
                );
                Ok(order_id)
            }
            ExchangeType::HyperliquidMainnet => {
                let client = self.client.as_ref().ok_or_else(|| anyhow!("Cliente no inicializado"))?;
                let price = order.price.ok_or_else(|| anyhow!("Precio requerido"))?;
                let side = match order.side {
                    crate::types::OrderSide::Buy => HyperliquidOrderSide::Buy,
                    crate::types::OrderSide::Sell => HyperliquidOrderSide::Sell,
                };
                let response = client
                    .place_limit_order(order.asset_index, side, order.quantity, price)
                    .await?;
                if response.success {
                    info!(
                        "💰 [MAINNET REAL] Orden colocada: {:?} {} @ ${:.2} - ID: {}",
                        order.side, order.quantity, price, response.order_id
                    );
                    Ok(response.order_id)
                } else {
                    Err(anyhow!("Error colocando orden: {}", response.message))
                }
            }
        }
    }

    pub async fn cancel_order(&self, order_id: &str) -> Result<()> {
        match self.exchange {
            ExchangeType::Simulated => {
                info!("🗑️ [SIMULADO] Cancelando orden: {}", order_id);
                Ok(())
            }
            ExchangeType::HyperliquidMainnet => {
                info!("🗑️ [MAINNET] Cancelando orden: {}", order_id);
                // TODO: implementar cancelación real si es necesario
                Ok(())
            }
        }
    }

    pub async fn get_position(&self, symbol: &str) -> Result<Option<Decimal>> {
        match self.exchange {
            ExchangeType::HyperliquidMainnet => {
                if let Some(client) = &self.client {
                    let state = client.get_account_state().await?;
                    Ok(state.balances.get(symbol).cloned())
                } else {
                    Ok(Some(Decimal::ZERO))
                }
            }
            _ => Ok(Some(Decimal::ZERO))
        }
    }

    pub async fn get_balance(&self, asset: &str) -> Result<Decimal> {
        match self.exchange {
            ExchangeType::HyperliquidMainnet => {
                if let Some(client) = &self.client {
                    client.get_balance(asset).await
                } else {
                    Ok(Decimal::ZERO)
                }
            }
            _ => Ok(Decimal::ZERO)
        }
    }

    pub async fn get_mid_price(&self, symbol: &str) -> Result<Option<Decimal>> {
        match self.exchange {
            ExchangeType::HyperliquidMainnet => {
                if let Some(client) = &self.client {
                    Ok(Some(client.get_mid_price(symbol).await?))
                } else {
                    Ok(None)
                }
            }
            _ => Ok(None)
        }
    }

    pub fn round_price(&self, symbol: &str, price: Decimal) -> Option<Decimal> {
        if let Some(client) = &self.client {
            client.round_price(symbol, price)
        } else {
            Some(price)
        }
    }

    pub fn round_size(&self, symbol: &str, size: Decimal) -> Option<Decimal> {
        if let Some(client) = &self.client {
            client.round_size(symbol, size)
        } else {
            Some(size)
        }
    }

    pub fn get_asset_index(&self, symbol: &str) -> Option<u32> {
        if let Some(client) = &self.client {
            client.get_market_info(symbol).map(|m| m.index as u32)
        } else {
            None
        }
    }

    pub fn is_mainnet(&self) -> bool {
        matches!(self.exchange, ExchangeType::HyperliquidMainnet)
    }

    pub async fn shutdown(&self) -> Result<()> {
        info!("📴 Executor apagado");
        Ok(())
    }
}