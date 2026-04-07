use dotenv::dotenv;
use std::env;
use anyhow::Result;
use tracing::{info, error};
use hyperliquid_mmaker_bot::hyperliquid::{HyperliquidClient, HyperliquidOrderSide};
use rust_decimal_macros::dec;

#[tokio::main]
async fn main() -> Result<()> {
    dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .init();

    let private_key = env::var("PRIVATE_KEY").expect("PRIVATE_KEY no configurada");
    let is_mainnet = true;

    info!("🔧 Inicializando cliente Hyperliquid Mainnet...");
    let client = HyperliquidClient::new(&private_key, is_mainnet).await?;

    let balance = client.get_balance("BTC").await?;
    info!("💰 Balance de BTC: {}", balance);

    let asset_index = 0;
    let side = HyperliquidOrderSide::Buy;
    let quantity = dec!(0.00001);
    let price = dec!(10000);

    info!("📝 Enviando orden de prueba: asset_index={}, {:?} {} @ ${}", 
          asset_index, side, quantity, price);

    match client.place_limit_order(asset_index, side, quantity, price).await {
        Ok(response) => {
            info!("✅ Orden enviada con éxito: {:?}", response);
        }
        Err(e) => {
            error!("❌ Error al enviar orden: {}", e);
        }
    }

    Ok(())
}