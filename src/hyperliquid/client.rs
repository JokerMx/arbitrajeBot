//! Cliente real para Hyperliquid usando la SDK oficial

use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use anyhow::{Result, anyhow};
use tracing::{info, error, warn};
use hyperliquid_rust_sdk::{Client, Config, Order, OrderSide, OrderType, TimeInForce, Wallet};
use std::sync::Arc;

use super::models::*;

/// Cliente real para Hyperliquid API
pub struct HyperliquidClient {
    client: Arc<Client>,
    wallet: Wallet,
    wallet_address: String,
    is_mainnet: bool,
}

impl HyperliquidClient {
    /// Crear nuevo cliente para Hyperliquid
    pub async fn new(private_key: &str, is_mainnet: bool) -> Result<Self> {
        info!("🔐 Inicializando cliente Hyperliquid...");
        
        // Decodificar private key (sin el prefijo 0x)
        let private_key_hex = private_key.trim_start_matches("0x");
        let private_key_bytes = hex::decode(private_key_hex)
            .map_err(|e| anyhow!("Error decodificando private key: {}", e))?;
        
        // Crear wallet desde la private key
        let wallet = Wallet::from_private_key(private_key_bytes)?;
        let wallet_address = format!("{:?}", wallet.address());
        
        info!("📍 Wallet address: {}", wallet_address);
        
        // Configurar cliente
        let config = if is_mainnet {
            Config::mainnet()
        } else {
            Config::testnet()
        };
        
        let client = Client::new(config, wallet.clone()).await?;
        
        Ok(Self {
            client: Arc::new(client),
            wallet,
            wallet_address,
            is_mainnet,
        })
    }
    
    /// Obtener balance de la wallet
    pub async fn get_balance(&self, asset: &str) -> Result<Decimal> {
        let state = self.client.exchange().clearinghouse_state().await?;
        
        for position in state.asset_positions {
            if position.position.coin == asset {
                let size = position.position.szi.parse::<f64>()?;
                return Ok(Decimal::from_f64(size).unwrap_or(Decimal::ZERO));
            }
        }
        
        Ok(Decimal::ZERO)
    }
    
    /// Obtener precio actual de un símbolo
    pub async fn get_price(&self, symbol: &str) -> Result<Decimal> {
        let mids = self.client.info().all_mids().await?;
        
        if let Some(price_str) = mids.get(symbol) {
            let price_f64 = price_str.parse::<f64>()?;
            return Ok(Decimal::from_f64(price_f64).unwrap_or(Decimal::ZERO));
        }
        
        Err(anyhow!("No se encontró precio para {}", symbol))
    }
    
    /// Colocar una orden límite
    pub async fn place_limit_order(
        &self,
        symbol: &str,
        side: HyperliquidOrderSide,
        quantity: Decimal,
        price: Decimal,
    ) -> Result<HyperliquidOrderResponse> {
        info!("📝 Colocando orden REAL en Hyperliquid {}: {} {} @ ${:.2}", 
              if self.is_mainnet { "MAINNET" else { "TESTNET" },
              if side == HyperliquidOrderSide::Buy { "COMPRA" } else { "VENTA" },
              quantity, price);
        
        let quantity_f64 = quantity.to_f64().unwrap_or(0.0);
        let price_f64 = price.to_f64().unwrap_or(0.0);
        
        let order = Order {
            asset: symbol.to_string(),
            is_buy: side == HyperliquidOrderSide::Buy,
            reduce_only: false,
            limit_px: price_f64,
            sz: quantity_f64,
            order_type: OrderType::Limit,
            time_in_force: TimeInForce::Gtc,
        };
        
        match self.client.exchange().place_order(order).await {
            Ok(response) => {
                info!("✅ Orden colocada exitosamente en Hyperliquid");
                Ok(HyperliquidOrderResponse {
                    success: true,
                    order_id: response,
                    message: "Orden colocada".to_string(),
                })
            }
            Err(e) => {
                error!("❌ Error colocando orden: {}", e);
                Err(anyhow!("Error colocando orden: {}", e))
            }
        }
    }
    
    /// Obtener estado de la cuenta
    pub async fn get_account_state(&self) -> Result<HyperliquidAccountState> {
        let state = self.client.exchange().clearinghouse_state().await?;
        
        let mut balances = std::collections::HashMap::new();
        
        for position in state.asset_positions {
            let size = position.position.szi.parse::<f64>()?;
            balances.insert(position.position.coin, Decimal::from_f64(size).unwrap_or(Decimal::ZERO));
        }
        
        Ok(HyperliquidAccountState {
            balances,
            wallet_address: self.wallet_address.clone(),
        })
    }
}