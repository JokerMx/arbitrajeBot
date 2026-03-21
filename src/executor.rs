//! Ejecutor de órdenes - Versión simulada con logs mejorados

use rust_decimal::Decimal;
use rust_decimal::prelude::FromPrimitive;  // <-- AÑADIDO
use tracing::info;
use anyhow::{Result, anyhow};
use tokio::sync::mpsc;
use hypersdk::hypercore::PrivateKeySigner;

use crate::types::OrderRequest;

enum ExecutorCommand {
    PlaceOrder(OrderRequest, tokio::sync::oneshot::Sender<Result<String>>),
    CancelOrder(String, tokio::sync::oneshot::Sender<Result<()>>),
    GetPosition(String, tokio::sync::oneshot::Sender<Option<Decimal>>),
    Shutdown,
}

pub struct OrderExecutor {
    _signer: PrivateKeySigner,
    command_tx: mpsc::UnboundedSender<ExecutorCommand>,
}

impl OrderExecutor {
    pub async fn new(private_key: &str, _is_testnet: bool) -> Result<Self> {
        let signer: PrivateKeySigner = private_key
            .parse()
            .map_err(|e| anyhow!("Error parsing private key: {}", e))?;

        let (command_tx, mut command_rx) = mpsc::unbounded_channel();

        tokio::spawn(async move {
            while let Some(cmd) = command_rx.recv().await {
                match cmd {
                    ExecutorCommand::PlaceOrder(_order, response_tx) => {  // <-- PREFIJO _
                        let result = Self::handle_place_order().await;  // <-- SIN PARÁMETRO
                        let _ = response_tx.send(result);
                    }
                    ExecutorCommand::CancelOrder(order_id, response_tx) => {
                        let result = Self::handle_cancel_order(&order_id).await;
                        let _ = response_tx.send(result);
                    }
                    ExecutorCommand::GetPosition(symbol, response_tx) => {
                        let result = Self::handle_get_position(&symbol).await;
                        let _ = response_tx.send(result);
                    }
                    ExecutorCommand::Shutdown => {
                        info!("📴 Executor recibió shutdown");
                        break;
                    }
                }
            }
        });

        Ok(Self { 
            _signer: signer,
            command_tx 
        })
    }

    // AHORA SIN PARÁMETRO (no lo usamos en simulación)
    async fn handle_place_order() -> Result<String> {
        // Simular un pequeño delay como si fuera una orden real
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        
        let order_id = format!("order_{}", uuid::Uuid::new_v4());
        Ok(order_id)
    }

    async fn handle_cancel_order(order_id: &str) -> Result<()> {
        info!("🗑️ [SIMULADO] Cancelando orden: {}", order_id);
        Ok(())
    }

    async fn handle_get_position(_symbol: &str) -> Option<Decimal> {  // <-- PREFIJO _
        // CORREGIDO: usar Decimal::from_f64 con el trait FromPrimitive importado
        Some(Decimal::from_f64(0.1).unwrap_or(Decimal::ZERO))
    }

    pub async fn place_order(&self, order: OrderRequest) -> Result<String> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.command_tx.send(ExecutorCommand::PlaceOrder(order, tx))?;
        rx.await?
    }

    pub async fn cancel_order(&self, order_id: &str) -> Result<()> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.command_tx.send(ExecutorCommand::CancelOrder(
            order_id.to_string(), 
            tx
        ))?;
        rx.await?
    }

    pub async fn get_position(&self, symbol: &str) -> Result<Option<Decimal>> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.command_tx.send(ExecutorCommand::GetPosition(
            symbol.to_string(), 
            tx
        ))?;
        Ok(rx.await?)
    }

    pub async fn shutdown(&self) -> Result<()> {
        self.command_tx.send(ExecutorCommand::Shutdown)?;
        Ok(())
    }
}

impl Drop for OrderExecutor {
    fn drop(&mut self) {
        let _ = self.command_tx.send(ExecutorCommand::Shutdown);
    }
}