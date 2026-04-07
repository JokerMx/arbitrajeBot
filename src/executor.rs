//! Ejecutor de órdenes reales usando hypersdk (EIP-712)

use anyhow::{Result, anyhow};
use rust_decimal::Decimal;
use tracing::info; // <-- AÑADIR ESTA LÍNEA
use uuid::Uuid;
use crate::types::OrderRequest;
use crate::hyperliquid::{HyperliquidClient, HyperliquidOrderSide};
use crate::bybit::{BybitClient, BybitOrderSide as BybitSide, BybitOrderType as BybitOrdType};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ExchangeType {
    Simulated,
    HyperliquidMainnet,
    Bybit,
}

pub struct OrderExecutor {
    exchange: ExchangeType,
    client: Option<HyperliquidClient>,
    bybit_client: Option<BybitClient>,
}

impl OrderExecutor {
    pub async fn new(private_key: &str, exchange: ExchangeType, api_key: Option<&str>, api_secret: Option<&str>, testnet: bool) -> Result<Self> {
        match exchange {
            ExchangeType::Simulated => {
                info!("🎮 Usando modo simulado");
                Ok(Self { exchange, client: None, bybit_client: None })
            }
            ExchangeType::HyperliquidMainnet => {
                info!("🌐 Conectando a Hyperliquid MAINNET con hypersdk...");
                let client = HyperliquidClient::new(private_key, true).await?;
                info!("✅ Cliente Hyperliquid inicializado (EIP-712)");
                Ok(Self { exchange, client: Some(client), bybit_client: None })
            }
            ExchangeType::Bybit => {
                if testnet {
                    info!("🧪 Conectando a Bybit TESTNET...");
                } else {
                    info!("🔵 Conectando a Bybit MAINNET...");
                }
                let key = api_key.ok_or_else(|| anyhow!("API Key requerida para Bybit"))?;
                let secret = api_secret.ok_or_else(|| anyhow!("API Secret requerida para Bybit"))?;
                let client = BybitClient::new(key, secret, testnet);
                info!("✅ Cliente Bybit inicializado (testnet: {})", testnet);
                Ok(Self { exchange, client: None, bybit_client: Some(client) })
            }
        }
    }

    pub async fn place_order(&self, order: OrderRequest) -> Result<String> {
        match self.exchange {
            ExchangeType::Simulated => {
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                let order_id = format!("sim_{}", Uuid::new_v4());
                info!(
                    "💰 [SIMULADO] Orden placed: {:?} {} @ ${:.2} - ID: {}",
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
                        "💰 [MAINNET REAL] Orden placed: {:?} {} @ ${:.2} - ID: {}",
                        order.side, order.quantity, price, response.order_id
                    );
                    Ok(response.order_id)
                } else {
                    Err(anyhow!("Error placing order: {}", response.message))
                }
            }
            ExchangeType::Bybit => {
                let client = self.bybit_client.as_ref().ok_or_else(|| anyhow!("Cliente Bybit no inicializado"))?;
                let price = order.price.ok_or_else(|| anyhow!("Precio requerido"))?;
                let symbol = self.symbol_to_bybit(&order.symbol);
                let side = match order.side {
                    crate::types::OrderSide::Buy => BybitSide::Buy,
                    crate::types::OrderSide::Sell => BybitSide::Sell,
                };
                let response = client.place_order(&symbol, side, BybitOrdType::Limit, order.quantity, Some(price)).await?;
                info!(
                    "💰 [BYBIT] Orden placed: {:?} {} @ ${:.2} - ID: {}",
                    order.side, order.quantity, price, response.order_id
                );
                Ok(response.order_id)
            }
        }
    }

    fn symbol_to_bybit(&self, symbol: &str) -> String {
        format!("{}USDT", symbol.to_uppercase())
    }

    pub async fn cancel_order(&self, order_id: &str) -> Result<()> {
        match self.exchange {
            ExchangeType::Simulated => {
                info!("🗑️ [SIMULADO] Cancelando orden: {}", order_id);
                Ok(())
            }
            ExchangeType::HyperliquidMainnet => {
                info!("🗑️ [MAINNET] Cancelando orden: {}", order_id);
                Ok(())
            }
            ExchangeType::Bybit => {
                info!("🗑️ [BYBIT] Cancelando orden: {}", order_id);
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
            ExchangeType::Bybit => {
                Ok(Some(Decimal::ZERO))
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
            ExchangeType::Bybit => {
                if let Some(client) = &self.bybit_client {
                    let balance: Decimal = client.get_balance(asset).await.unwrap_or(Decimal::ZERO);
                    Ok(balance)
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
            ExchangeType::Bybit => {
                if let Some(client) = &self.bybit_client {
                    let bybit_symbol = self.symbol_to_bybit(symbol);
                    Ok(Some(client.get_mid_price(&bybit_symbol).await?))
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

    pub fn is_bybit(&self) -> bool {
        matches!(self.exchange, ExchangeType::Bybit)
    }

    pub async fn shutdown(&self) -> Result<()> {
        info!("📴 Executor apagado");
        Ok(())
    }
}