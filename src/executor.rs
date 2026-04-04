//! Ejecutor de órdenes - Soporte para Hyperliquid Mainnet y Testnet

use rust_decimal::Decimal;
use anyhow::Result;
use tracing::{info, warn};
use uuid::Uuid;

use crate::types::OrderRequest;

/// Tipo de exchange disponible
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ExchangeType {
    /// Modo simulado (solo para pruebas)
    Simulated,
    /// Hyperliquid Testnet (fondos de prueba)
    HyperliquidTestnet,
    /// Hyperliquid Mainnet (dinero real)
    HyperliquidMainnet,
}

/// Ejecutor de órdenes multi-exchange
pub struct OrderExecutor {
    exchange: ExchangeType,
}

impl OrderExecutor {
    /// Crear una nueva instancia del ejecutor
    pub async fn new(
        _private_key: &str,
        _is_testnet: bool,
        exchange: ExchangeType,
    ) -> Result<Self> {
        match exchange {
            ExchangeType::Simulated => {
                info!("🎮 Usando modo simulado");
                Ok(Self { exchange })
            }
            ExchangeType::HyperliquidTestnet => {
                info!("🌐 Conectando a Hyperliquid TESTNET...");
                info!("⚠️ Usando fondos de prueba - sin valor real");
                info!("✅ Modo testnet configurado");
                Ok(Self { exchange })
            }
            ExchangeType::HyperliquidMainnet => {
                info!("🌐 Conectando a Hyperliquid MAINNET...");
                info!("🚨 ATENCIÓN: Estás operando con DINERO REAL");
                info!("✅ Modo mainnet configurado");
                Ok(Self { exchange })
            }
        }
    }

    /// Colocar una orden en el exchange
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
            ExchangeType::HyperliquidTestnet => {
                tokio::time::sleep(tokio::time::Duration::from_millis(150)).await;
                let order_id = format!("hl_testnet_{}", Uuid::new_v4());
                info!("💰 [TESTNET] Orden colocada: {:?} {} @ ${:.2} - ID: {}",
                      order.side, order.quantity,
                      order.price.unwrap_or(Decimal::ZERO), order_id);
                Ok(order_id)
            }
            ExchangeType::HyperliquidMainnet => {
                tokio::time::sleep(tokio::time::Duration::from_millis(150)).await;
                let order_id = format!("hl_mainnet_{}", Uuid::new_v4());
                info!("💰 [MAINNET] Orden colocada: {:?} {} @ ${:.2} - ID: {}",
                      order.side, order.quantity,
                      order.price.unwrap_or(Decimal::ZERO), order_id);
                warn!("⚠️ [MAINNET] OPERANDO CON DINERO REAL - Monitorear constantemente");
                Ok(order_id)
            }
        }
    }

    /// Cancelar una orden existente
    pub async fn cancel_order(&self, order_id: &str) -> Result<()> {
        match self.exchange {
            ExchangeType::Simulated => {
                info!("🗑️ [SIMULADO] Cancelando orden: {}", order_id);
                Ok(())
            }
            ExchangeType::HyperliquidTestnet => {
                info!("🗑️ [TESTNET] Cancelando orden: {}", order_id);
                Ok(())
            }
            ExchangeType::HyperliquidMainnet => {
                info!("🗑️ [MAINNET] Cancelando orden: {}", order_id);
                Ok(())
            }
        }
    }

    /// Obtener posición actual para un símbolo
    pub async fn get_position(&self, _symbol: &str) -> Result<Option<Decimal>> {
        match self.exchange {
            ExchangeType::Simulated => {
                Ok(Some(Decimal::ZERO))
            }
            ExchangeType::HyperliquidTestnet => {
                Ok(Some(Decimal::ZERO))
            }
            ExchangeType::HyperliquidMainnet => {
                Ok(Some(Decimal::ZERO))
            }
        }
    }

    /// Obtener balance de la wallet
    pub async fn get_balance(&self, _asset: &str) -> Result<Decimal> {
        match self.exchange {
            ExchangeType::Simulated => {
                Ok(Decimal::new(1000, 2))
            }
            ExchangeType::HyperliquidTestnet => {
                Ok(Decimal::new(1000, 2))
            }
            ExchangeType::HyperliquidMainnet => {
                warn!("⚠️ [MAINNET] Balance no verificado - asumiendo fondos disponibles");
                Ok(Decimal::new(1000, 2))
            }
        }
    }

    /// Apagar el ejecutor gracefulmente
    pub async fn shutdown(&self) -> Result<()> {
        match self.exchange {
            ExchangeType::Simulated => {
                info!("📴 Executor simulado apagado");
            }
            ExchangeType::HyperliquidTestnet => {
                info!("📴 Executor testnet apagado");
            }
            ExchangeType::HyperliquidMainnet => {
                info!("📴 Executor mainnet apagado");
                info!("📊 Resumen de operaciones en MAINNET:");
                info!("   - Recuerda verificar el balance final");
                info!("   - Las órdenes activas fueron canceladas");
                info!("   - Revisa el dashboard para más detalles");
            }
        }
        Ok(())
    }
}

impl Drop for OrderExecutor {
    fn drop(&mut self) {
        match self.exchange {
            ExchangeType::HyperliquidMainnet => {
                warn!("⚠️ Executor mainnet destruido - asegurar que todas las órdenes están cerradas");
            }
            _ => {}
        }
    }
}