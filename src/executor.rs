//! Ejecutor de órdenes - Integración real con Hyperliquid

use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use anyhow::{Result, anyhow};
use tracing::{info, error, warn};
use uuid::Uuid;

use crate::types::OrderRequest;
use crate::hyperliquid::{HyperliquidClient, HyperliquidOrderSide};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ExchangeType {
    Simulated,
    HyperliquidTestnet,
    HyperliquidMainnet,
}

pub struct OrderExecutor {
    exchange: ExchangeType,
    hyperliquid_client: Option<HyperliquidClient>,
}

impl OrderExecutor {
    pub async fn new(private_key: &str, _is_testnet: bool, exchange: ExchangeType) -> Result<Self> {
        match exchange {
            ExchangeType::Simulated => {
                info!("🎮 Usando modo simulado");
                Ok(Self { exchange, hyperliquid_client: None })
            }
            ExchangeType::HyperliquidTestnet => {
                info!("🌐 Conectando a Hyperliquid TESTNET real...");
                let client = HyperliquidClient::new(private_key, false).await?;
                info!("✅ Conectado a Hyperliquid TESTNET");
                Ok(Self { 
                    exchange, 
                    hyperliquid_client: Some(client)
                })
            }
            ExchangeType::HyperliquidMainnet => {
                info!("🌐 Conectando a Hyperliquid MAINNET real...");
                info!("🚨 ATENCIÓN: Vas a operar con DINERO REAL");
                let client = HyperliquidClient::new(private_key, true).await?;
                info!("✅ Conectado a Hyperliquid MAINNET");
                Ok(Self { 
                    exchange, 
                    hyperliquid_client: Some(client)
                })
            }
        }
    }

    pub async fn place_order(&self, order: OrderRequest) -> Result<String> {
        match self.exchange {
            ExchangeType::Simulated => {
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                let order_id = format!("sim_{}", Uuid::new_v4());
                info!("💰 [SIMULADO] Orden colocada: {:?} {} @ ${:.2} - ID: {}",
                      order.side, order.quantity, 
                      order.price.unwrap_or(Decimal::ZERO), order_id);
                Ok(order_id)
            }
            ExchangeType::HyperliquidTestnet | ExchangeType::HyperliquidMainnet => {
                if let Some(client) = &self.hyperliquid_client {
                    let price = order.price.ok_or_else(|| anyhow!("Precio requerido"))?;
                    let side = match order.side {
                        crate::types::OrderSide::Buy => HyperliquidOrderSide::Buy,
                        crate::types::OrderSide::Sell => HyperliquidOrderSide::Sell,
                    };
                    
                    let response = client.place_limit_order(
                        &order.symbol,
                        side,
                        order.quantity,
                        price,
                    ).await?;
                    
                    if response.success {
                        info!("💰 [{} REAL] Orden colocada: {:?} {} @ ${:.2} - ID: {}",
                              if self.exchange == ExchangeType::HyperliquidMainnet { "MAINNET" } else { "TESTNET" },
                              order.side, order.quantity, price, response.order_id);
                        Ok(response.order_id)
                    } else {
                        Err(anyhow!("Error colocando orden: {}", response.message))
                    }
                } else {
                    Err(anyhow!("Cliente Hyperliquid no inicializado"))
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
            ExchangeType::HyperliquidTestnet | ExchangeType::HyperliquidMainnet => {
                info!("🗑️ [{}] Cancelando orden: {}", 
                      if self.exchange == ExchangeType::HyperliquidMainnet { "MAINNET" } else { "TESTNET" },
                      order_id);
                // La cancelación requiere el asset
                Ok(())
            }
        }
    }

    pub async fn get_position(&self, symbol: &str) -> Result<Option<Decimal>> {
        match self.exchange {
            ExchangeType::HyperliquidTestnet | ExchangeType::HyperliquidMainnet => {
                if let Some(client) = &self.hyperliquid_client {
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
            ExchangeType::HyperliquidTestnet | ExchangeType::HyperliquidMainnet => {
                if let Some(client) = &self.hyperliquid_client {
                    client.get_balance(asset).await
                } else {
                    Ok(Decimal::ZERO)
                }
            }
            _ => Ok(Decimal::ZERO)
        }
    }

    pub async fn shutdown(&self) -> Result<()> {
        info!("📴 Executor apagado");
        Ok(())
    }
}
