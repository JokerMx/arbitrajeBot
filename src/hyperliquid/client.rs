//! Cliente real para Hyperliquid usando hypersdk (API REST + firma EIP-712)
//! https://github.com/infinitefield/hypersdk

use anyhow::{Result, anyhow};
use rust_decimal::Decimal;
use rust_decimal::prelude::FromPrimitive;
use rust_decimal::RoundingStrategy;
use tracing::{info, error, warn};

use hypersdk::hypercore::{self, types::*, PrivateKeySigner, NonceHandler};
use hypersdk::Address;

use super::models::*;

/// Valor mínimo de orden en Hyperliquid (USD)
pub const MIN_ORDER_VALUE_USD: f64 = 10.0;
/// Slippage máximo de un stop-loss market disparado
pub const STOP_SLIPPAGE: Decimal = rust_decimal_macros::dec!(0.03);

// ------------------------------------------------------------
// Cliente principal - usa hypersdk para Hyperliquid Mainnet
// ------------------------------------------------------------
pub struct HyperliquidClient {
    http_client: hypercore::HttpClient,
    signer: PrivateKeySigner,
    wallet_address: Address,
    nonce_handler: NonceHandler,
    _is_mainnet: bool,
    cached_spot_tokens: Vec<hypercore::SpotToken>,
    cached_markets: std::collections::HashMap<String, MarketInfo>,
}

impl HyperliquidClient {
    pub async fn new(private_key: &str, is_mainnet: bool) -> Result<Self> {
        let client = if is_mainnet {
            hypercore::mainnet()
        } else {
            hypercore::testnet()
        };

        let signer: PrivateKeySigner = private_key.parse()
            .map_err(|e| anyhow!("Invalid private key: {}", e))?;
        
        // Con una API wallet (agente) la clave firma pero la cuenta con fondos es otra:
        // WALLET indica la cuenta principal a consultar. Un agente no puede retirar fondos.
        let wallet_address = match std::env::var("WALLET").ok().filter(|w| !w.trim().is_empty()) {
            Some(w) => {
                let addr: Address = w.trim().parse().map_err(|e| anyhow!("WALLET inválida: {}", e))?;
                if addr != signer.address() {
                    info!("🔐 Modo API wallet: firma {} en nombre de {}", signer.address(), addr);
                }
                addr
            }
            None => signer.address(),
        };

        info!("📍 Wallet address: {}", wallet_address);
        info!("🌐 Network: {}", if is_mainnet { "mainnet" } else { "testnet" });

        let cached_spot_tokens = client.spot_tokens().await.unwrap_or_default();
        
        let mut cached_markets = std::collections::HashMap::new();
        let perps = client.perps().await.unwrap_or_default();
        for market in perps {
            cached_markets.insert(market.name.clone(), MarketInfo {
                name: market.name.clone(),
                index: market.index,
                max_leverage: market.max_leverage,
                sz_decimals: market.sz_decimals,
            });
        }
        
        info!("📊 Cacheados {} mercados perpetuos", cached_markets.len());

        Ok(Self {
            http_client: client,
            signer,
            wallet_address,
            nonce_handler: NonceHandler::default(),
            _is_mainnet: is_mainnet,
            cached_spot_tokens,
            cached_markets,
        })
    }

    /// Nombres de todos los perpetuos de Hyperliquid, ordenados
    pub fn market_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self.cached_markets.keys().cloned().collect();
        names.sort();
        names
    }

    pub fn get_market_info(&self, symbol: &str) -> Option<&MarketInfo> {
        self.cached_markets.get(symbol)
    }
    
    pub fn get_symbol_for_index(&self, index: u32) -> String {
        for (name, info) in &self.cached_markets {
            if info.index == index as usize {
                return name.clone();
            }
        }
        format!("unknown_{}", index)
    }

    fn find_spot_token(&self, asset: &str) -> Option<hypercore::SpotToken> {
        self.cached_spot_tokens.iter()
            .find(|t| t.name.eq_ignore_ascii_case(asset))
            .cloned()
    }

    // --------------------------------------------------------
    // Consultas a la API (info)
    // --------------------------------------------------------
    pub async fn get_balance(&self, asset: &str) -> Result<Decimal> {
        let state = self.http_client.clearinghouse_state(self.wallet_address, None).await?;
        
        for asset_pos in &state.asset_positions {
            let position = &asset_pos.position;
            if &position.coin == asset {
                return Ok(position.szi);
            }
        }
        Ok(Decimal::ZERO)
    }

    pub async fn get_account_state(&self) -> Result<HyperliquidAccountState> {
        let state = self.http_client.clearinghouse_state(self.wallet_address, None).await?;
        
        let mut balances = std::collections::HashMap::new();
        
        for asset_pos in &state.asset_positions {
            let position = &asset_pos.position;
            balances.insert(position.coin.clone(), position.szi);
        }
        
        Ok(HyperliquidAccountState {
            balances,
            wallet_address: self.wallet_address.to_string(),
        })
    }

    // --------------------------------------------------------
    // Obtener mercados perpetuos
    // --------------------------------------------------------
    pub async fn get_perpetual_markets(&self) -> Result<Vec<MarketInfo>> {
        let markets = self.http_client.perps().await?;
        
        let mut result = Vec::new();
        for market in markets {
            result.push(MarketInfo {
                name: market.name.clone(),
                index: market.index,
                max_leverage: market.max_leverage,
                sz_decimals: market.sz_decimals,
            });
        }
        
        Ok(result)
    }

    // --------------------------------------------------------
    // Obtener mercados spot
    // --------------------------------------------------------
    pub async fn get_spot_markets(&self) -> Result<Vec<SpotMarketInfo>> {
        let markets = self.http_client.spot().await?;
        
        let mut result = Vec::new();
        for market in markets {
            result.push(SpotMarketInfo {
                name: market.symbol(),
                index: market.index,
            });
        }
        
        Ok(result)
    }

    // --------------------------------------------------------
    // Colocar orden límite (GTC, compatible con código existente)
    // --------------------------------------------------------
    pub async fn place_limit_order(
        &self,
        asset_index: u32,
        side: HyperliquidOrderSide,
        quantity: Decimal,
        price: Decimal,
    ) -> Result<HyperliquidOrderResponse> {
        self.place_order_ext(asset_index, side, quantity, price, TimeInForce::Gtc, false).await
    }

    // --------------------------------------------------------
    // Colocar orden con TIF y reduce_only explícitos.
    // Devuelve el oid REAL del exchange (necesario para cancelar).
    // --------------------------------------------------------
    pub async fn place_order_ext(
        &self,
        asset_index: u32,
        side: HyperliquidOrderSide,
        quantity: Decimal,
        price: Decimal,
        tif: TimeInForce,
        reduce_only: bool,
    ) -> Result<HyperliquidOrderResponse> {
        let is_buy = side == HyperliquidOrderSide::Buy;

        let symbol = self.get_symbol_for_index(asset_index);
        // Redondeo direccional: una compra nunca se redondea hacia arriba ni una venta hacia abajo
        let rounded_price = self.round_price_dir(&symbol, price, !is_buy).unwrap_or(price);
        let rounded_size = self.round_size(&symbol, quantity).unwrap_or(quantity);

        let order_value = rounded_size * rounded_price;
        let min_value = Decimal::from_f64(MIN_ORDER_VALUE_USD).unwrap();

        // Las órdenes reduce_only cierran una posición existente: no se infla su tamaño
        let final_size = if !reduce_only && order_value < min_value {
            let min_size = min_value / rounded_price;
            self.round_size_up(&symbol, min_size).unwrap_or(min_size)
        } else {
            rounded_size
        };

        if final_size <= Decimal::ZERO {
            return Err(anyhow!("Tamaño de orden inválido para {}: {}", symbol, final_size));
        }

        let final_value = final_size * rounded_price;

        info!("📤 Placing order: {} {} size={} price={} value=${:.2} tif={:?} reduce_only={}",
            symbol, if is_buy { "BUY" } else { "SELL" }, final_size, rounded_price, final_value, tif, reduce_only);

        let order = BatchOrder {
            orders: vec![OrderRequest {
                asset: asset_index as usize,
                is_buy,
                limit_px: rounded_price,
                sz: final_size,
                reduce_only,
                order_type: OrderTypePlacement::Limit { tif },
                cloid: Default::default(),
            }],
            grouping: OrderGrouping::Na,
        };

        let nonce = self.nonce_handler.next();

        let result = self.http_client.place(&self.signer, order, nonce, None, None).await
            .map_err(|e| {
                error!("❌ Order failed: asset={}, error={}", asset_index, e);
                anyhow!("Order failed: {}", e)
            })?;

        for status in &result {
            match status {
                OrderResponseStatus::Resting { oid, .. } => {
                    info!("✅ Orden en libro oid={}", oid);
                    return Ok(HyperliquidOrderResponse {
                        success: true,
                        order_id: oid.to_string(),
                        message: "Resting".to_string(),
                    });
                }
                OrderResponseStatus::Filled { oid, total_sz, avg_px } => {
                    info!("✅ Orden ejecutada oid={} sz={} px={}", oid, total_sz, avg_px);
                    return Ok(HyperliquidOrderResponse {
                        success: true,
                        order_id: oid.to_string(),
                        message: "Filled".to_string(),
                    });
                }
                OrderResponseStatus::Error(msg) => {
                    // Con ALO (post-only) es normal que se rechace si cruzaría el libro
                    warn!("⚠️ Orden rechazada: {}", msg);
                    return Err(anyhow!("Order rejected: {}", msg));
                }
                _ => {}
            }
        }

        Err(anyhow!("Respuesta de orden sin oid: {:?}", result))
    }

    // --------------------------------------------------------
    // Cancelar orden por oid real
    // --------------------------------------------------------
    pub async fn cancel_order(&self, asset_index: u32, oid: u64) -> Result<()> {
        let batch = BatchCancel {
            cancels: vec![Cancel { asset: asset_index as usize, oid }],
        };
        let nonce = self.nonce_handler.next();
        let result = self.http_client.cancel(&self.signer, batch, nonce, None, None).await
            .map_err(|e| anyhow!("Cancel failed: {}", e))?;
        for status in &result {
            if let OrderResponseStatus::Error(msg) = status {
                // Típicamente: la orden ya se ejecutó o ya estaba cancelada
                warn!("⚠️ Cancel oid={}: {}", oid, msg);
            }
        }
        Ok(())
    }

    // --------------------------------------------------------
    // Cancelar TODAS las órdenes abiertas de un símbolo (un solo request)
    // --------------------------------------------------------
    pub async fn cancel_all_for(&self, symbol: &str) -> Result<usize> {
        let asset = self
            .get_market_info(symbol)
            .map(|m| m.index)
            .ok_or_else(|| anyhow!("Mercado {} desconocido", symbol))?;
        let open = self.http_client.open_orders(self.wallet_address, None).await?;
        let cancels: Vec<Cancel> = open
            .iter()
            .filter(|o| o.coin == symbol)
            .map(|o| Cancel { asset, oid: o.oid })
            .collect();
        let n = cancels.len();
        if n == 0 {
            return Ok(0);
        }
        let nonce = self.nonce_handler.next();
        self.http_client
            .cancel(&self.signer, BatchCancel { cancels }, nonce, None, None)
            .await
            .map_err(|e| anyhow!("Cancel all failed: {}", e))?;
        info!("🗑️ Canceladas {} órdenes abiertas de {}", n, symbol);
        Ok(n)
    }

    // --------------------------------------------------------
    // Posición actual: (tamaño con signo, precio de entrada)
    // --------------------------------------------------------
    pub async fn get_position(&self, symbol: &str) -> Result<(Decimal, Decimal)> {
        let state = self.http_client.clearinghouse_state(self.wallet_address, None).await?;
        for ap in &state.asset_positions {
            if ap.position.coin == symbol {
                return Ok((ap.position.szi, ap.position.entry_px.unwrap_or(Decimal::ZERO)));
            }
        }
        Ok((Decimal::ZERO, Decimal::ZERO))
    }

    // --------------------------------------------------------
    // Equity real de la cuenta (cross margin)
    // --------------------------------------------------------
    pub async fn get_account_value(&self) -> Result<Decimal> {
        let state = self.http_client.clearinghouse_state(self.wallet_address, None).await?;
        Ok(state.cross_margin_summary.account_value)
    }

    // --------------------------------------------------------
    // Cerrar la posición completa con orden IOC reduce_only agresiva
    // (equivale a "market close"; paga fee taker)
    // --------------------------------------------------------
    pub async fn close_position(&self, symbol: &str, max_slippage: Decimal) -> Result<()> {
        let (szi, _) = self.get_position(symbol).await?;
        if szi.is_zero() {
            return Ok(());
        }
        let asset = self
            .get_market_info(symbol)
            .map(|m| m.index as u32)
            .ok_or_else(|| anyhow!("Mercado {} desconocido", symbol))?;
        let mid = self.get_mid_price(symbol).await?;
        let (side, px) = if szi > Decimal::ZERO {
            (HyperliquidOrderSide::Sell, mid * (Decimal::ONE - max_slippage))
        } else {
            (HyperliquidOrderSide::Buy, mid * (Decimal::ONE + max_slippage))
        };
        warn!("🔻 Cerrando posición {} de {} a mercado (px límite {})", szi, symbol, px);
        self.place_order_ext(asset, side, szi.abs(), px, TimeInForce::Ioc, true).await?;
        Ok(())
    }

    // --------------------------------------------------------
    // Foto de la cuenta: equity y posiciones (coin -> (tamaño, entrada))
    // en un solo request
    // --------------------------------------------------------
    pub async fn get_account_snapshot(
        &self,
    ) -> Result<(Decimal, std::collections::HashMap<String, (Decimal, Decimal)>)> {
        let state = self.http_client.clearinghouse_state(self.wallet_address, None).await?;
        let mut positions = std::collections::HashMap::new();
        for ap in &state.asset_positions {
            let p = &ap.position;
            if !p.szi.is_zero() {
                positions.insert(p.coin.clone(), (p.szi, p.entry_px.unwrap_or(Decimal::ZERO)));
            }
        }
        Ok((state.cross_margin_summary.account_value, positions))
    }

    // --------------------------------------------------------
    // Stop-loss en el EXCHANGE (trigger market reduce-only).
    // Protege la posición aunque el bot esté apagado.
    // `long_position = true` -> el stop vende.
    // Devuelve el oid si se pudo identificar.
    // --------------------------------------------------------
    pub async fn place_stop_loss(
        &self,
        symbol: &str,
        long_position: bool,
        size: Decimal,
        trigger_px: Decimal,
    ) -> Result<Option<u64>> {
        let asset = self
            .get_market_info(symbol)
            .map(|m| m.index)
            .ok_or_else(|| anyhow!("Mercado {} desconocido", symbol))?;
        let is_buy = !long_position;
        // El trigger de un stop de venta se redondea hacia abajo (de compra, hacia arriba)
        let trigger = self.round_price_dir(symbol, trigger_px, is_buy).unwrap_or(trigger_px);
        // Precio límite = cota de slippage de la orden market disparada
        let limit = if is_buy {
            trigger * (Decimal::ONE + STOP_SLIPPAGE)
        } else {
            trigger * (Decimal::ONE - STOP_SLIPPAGE)
        };
        let limit = self.round_price_dir(symbol, limit, is_buy).unwrap_or(limit);
        let size = self.round_size(symbol, size).unwrap_or(size);

        let order = BatchOrder {
            orders: vec![OrderRequest {
                asset,
                is_buy,
                limit_px: limit,
                sz: size,
                reduce_only: true,
                order_type: OrderTypePlacement::Trigger {
                    is_market: true,
                    trigger_px: trigger,
                    tpsl: TpSl::Sl,
                },
                cloid: Default::default(),
            }],
            grouping: OrderGrouping::Na,
        };
        // Stops existentes antes de colocar (para identificar el nuevo si hace falta)
        let before: Vec<u64> = self
            .get_stop_orders(symbol)
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|(oid, _)| oid)
            .collect();
        let nonce = self.nonce_handler.next();
        info!("🛡️ Stop-loss {} size={} trigger={} limit={}", symbol, size, trigger, limit);

        match self.http_client.place(&self.signer, order, nonce, None, None).await {
            Ok(statuses) => {
                for st in &statuses {
                    match st {
                        OrderResponseStatus::Resting { oid, .. } => return Ok(Some(*oid)),
                        OrderResponseStatus::Error(msg) => {
                            return Err(anyhow!("Stop-loss rechazado: {}", msg))
                        }
                        _ => {}
                    }
                }
            }
            // Algunas versiones del SDK no reconocen la respuesta de órdenes trigger:
            // la orden puede haberse creado igual, se verifica abajo.
            Err(e) => warn!("Respuesta de stop-loss no reconocida ({}), verificando...", e),
        }

        let stops = self.get_stop_orders(symbol).await?;
        stops
            .iter()
            .find(|(oid, _)| !before.contains(oid))
            .map(|(oid, _)| Some(*oid))
            .ok_or_else(|| anyhow!("No se encontró el stop-loss de {} en el exchange", symbol))
    }

    // --------------------------------------------------------
    // Órdenes reduce-only abiertas de un símbolo: (oid, precio límite).
    // En modo tendencia las únicas órdenes reduce-only que quedan en el
    // libro son los stops del bot.
    // --------------------------------------------------------
    pub async fn get_stop_orders(&self, symbol: &str) -> Result<Vec<(u64, Decimal)>> {
        let open = self.http_client.open_orders(self.wallet_address, None).await?;
        Ok(open
            .iter()
            .filter(|o| o.coin == symbol && o.reduce_only)
            .map(|o| (o.oid, o.limit_px))
            .collect())
    }

    // --------------------------------------------------------
    // Obtener órdenes activas del usuario
    // --------------------------------------------------------
    pub async fn get_open_orders(&self) -> Result<Vec<OpenOrder>> {
        let open_orders = self.http_client.open_orders(self.wallet_address, None).await?;
        
        let mut orders = Vec::new();
        
        for open_order in open_orders {
            orders.push(OpenOrder {
                coin: open_order.coin.clone(),
                oid: open_order.oid,
                side: open_order.side.to_string(),
                price: open_order.limit_px,
                size: open_order.sz,
            });
        }
        
        Ok(orders)
    }

    // --------------------------------------------------------
    // Transferir a HyperEVM
    // --------------------------------------------------------
    pub async fn transfer_to_evm(&self, amount: Decimal, asset: &str) -> Result<()> {
        let token = self.find_spot_token(asset)
            .ok_or_else(|| anyhow!("Token {} not found", asset))?;
        
        let nonce = self.nonce_handler.next();
        
        self.http_client.transfer_to_evm(&self.signer, token, amount, nonce).await
            .map_err(|e| anyhow!("Transfer to EVM failed: {}", e))?;
        
        info!("✅ Transferido {} {} a EVM", amount, asset);
        Ok(())
    }

    // --------------------------------------------------------
    // Transferir desde HyperEVM - no disponible en SDK
    // --------------------------------------------------------
    pub async fn transfer_from_evm(&self, _amount: Decimal, _asset: &str) -> Result<()> {
        warn!("Transfer from EVM no disponible en SDK. Usa la UI.");
        Err(anyhow!("Use Hyperliquid UI for transfers from EVM"))
    }

    // --------------------------------------------------------
    // Transferir a perpetuos (desde spot)
    // --------------------------------------------------------
    pub async fn transfer_to_perps(&self, amount: Decimal, asset: &str) -> Result<()> {
        let token = self.find_spot_token(asset)
            .ok_or_else(|| anyhow!("Token {} not found", asset))?;
        
        let nonce = self.nonce_handler.next();
        
        self.http_client.transfer_to_perps(&self.signer, token, amount, nonce).await
            .map_err(|e| anyhow!("Transfer to perps failed: {}", e))?;
        
        info!("✅ Transferido {} {} a perpetuos", amount, asset);
        Ok(())
    }

    // --------------------------------------------------------
    // Transferir a spot (desde perpetuos)
    // --------------------------------------------------------
    pub async fn transfer_to_spot(&self, amount: Decimal, asset: &str) -> Result<()> {
        let token = self.find_spot_token(asset)
            .ok_or_else(|| anyhow!("Token {} not found", asset))?;
        
        let nonce = self.nonce_handler.next();
        
        self.http_client.transfer_to_spot(&self.signer, token, amount, nonce).await
            .map_err(|e| anyhow!("Transfer to spot failed: {}", e))?;
        
        info!("✅ Transferido {} {} a spot", amount, asset);
        Ok(())
    }

    // --------------------------------------------------------
    // Obtener precio actual de un mercado
    // --------------------------------------------------------
    pub async fn get_mid_price(&self, asset: &str) -> Result<Decimal> {
        let all_mids = self.http_client.all_mids(None).await?;
        
        all_mids.get(asset)
            .ok_or_else(|| anyhow!("No price found for {}", asset))
            .map(|p| *p)
    }

    // --------------------------------------------------------
    // Obtener todos los precios medios
    // --------------------------------------------------------
    pub async fn get_all_mids(&self) -> Result<std::collections::HashMap<String, Decimal>> {
        self.http_client.all_mids(None).await
    }

    // --------------------------------------------------------
    // Decimales de precio permitidos por Hyperliquid (perps):
    // máximo (6 - szDecimals) decimales Y máximo 5 cifras significativas
    // --------------------------------------------------------
    fn price_decimals(&self, symbol: &str, price: Decimal) -> Option<u32> {
        let market = self.cached_markets.get(symbol)?;
        let max_by_sz = (6 - market.sz_decimals).max(0);
        let int_digits = if price.abs() < Decimal::ONE {
            0
        } else {
            price.abs().trunc().to_string().len() as i64
        };
        let max_by_sig = (5 - int_digits).max(0);
        Some(max_by_sz.min(max_by_sig).min(8) as u32)
    }

    pub fn round_price(&self, symbol: &str, price: Decimal) -> Option<Decimal> {
        let dp = self.price_decimals(symbol, price)?;
        Some(price.round_dp(dp))
    }

    /// Redondeo direccional: `up = true` hacia arriba (asks), `false` hacia abajo (bids)
    pub fn round_price_dir(&self, symbol: &str, price: Decimal, up: bool) -> Option<Decimal> {
        let dp = self.price_decimals(symbol, price)?;
        let strategy = if up {
            RoundingStrategy::ToPositiveInfinity
        } else {
            RoundingStrategy::ToNegativeInfinity
        };
        Some(price.round_dp_with_strategy(dp, strategy))
    }

    // --------------------------------------------------------
    // Redondear tamaño según decimales del mercado
    // --------------------------------------------------------
    pub fn round_size(&self, symbol: &str, size: Decimal) -> Option<Decimal> {
        let market = self.cached_markets.get(symbol)?;
        Some(size.round_dp(market.sz_decimals.max(0) as u32))
    }

    fn round_size_up(&self, symbol: &str, size: Decimal) -> Option<Decimal> {
        let market = self.cached_markets.get(symbol)?;
        Some(size.round_dp_with_strategy(
            market.sz_decimals.max(0) as u32,
            RoundingStrategy::ToPositiveInfinity,
        ))
    }
}

// ------------------------------------------------------------
// Tipos adicionales para el cliente
// ------------------------------------------------------------
#[derive(Debug, Clone)]
pub struct MarketInfo {
    pub name: String,
    pub index: usize,
    pub max_leverage: u64,
    pub sz_decimals: i64,
}

#[derive(Debug, Clone)]
pub struct SpotMarketInfo {
    pub name: String,
    pub index: usize,
}

#[derive(Debug, Clone)]
pub struct OpenOrder {
    pub coin: String,
    pub oid: u64,
    pub side: String,
    pub price: Decimal,
    pub size: Decimal,
}