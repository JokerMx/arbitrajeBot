//! Cliente real para Hyperliquid usando API REST con firma ECDSA (secp256k1)

use rust_decimal::Decimal;
use rust_decimal::prelude::{FromPrimitive, ToPrimitive};
use anyhow::{Result, anyhow};
use tracing::{info, error, warn};
use reqwest::Client as HttpClient;
use serde_json::json;
use std::collections::{HashMap, BTreeMap};
use std::sync::atomic::{AtomicU64, Ordering};
use secp256k1::{Secp256k1, SecretKey, Message};
use sha3::{Digest, Keccak256};
use ethers::core::k256::ecdsa::SigningKey;
use ethers::utils::secret_key_to_address;

use super::models::*;

/// Cliente real para Hyperliquid API
pub struct HyperliquidClient {
    http_client: HttpClient,
    wallet_address: String,
    private_key_bytes: Vec<u8>,
    base_url: String,
    is_mainnet: bool,
    nonce_counter: AtomicU64,
}

impl HyperliquidClient {
    pub async fn new(private_key: &str, is_mainnet: bool) -> Result<Self> {
        let base_url = if is_mainnet {
            "https://api.hyperliquid.xyz"
        } else {
            "https://api.hyperliquid-testnet.xyz"
        };
        let private_key_hex = private_key.trim_start_matches("0x");
        let private_key_bytes = hex::decode(private_key_hex)?;
        
        // Derivar dirección correctamente usando ethers
        let private_key_array: [u8; 32] = private_key_bytes.as_slice().try_into()
            .map_err(|_| anyhow!("Clave privada debe tener 32 bytes"))?;
        let signing_key = SigningKey::from_bytes(&private_key_array.into())?;
        let address = secret_key_to_address(&signing_key);
        let wallet_address = format!("{:?}", address); // Con 0x y checksum

        info!("📍 Wallet address: {}", wallet_address);
        info!("🌐 API URL: {}", base_url);

        Ok(Self {
            http_client: HttpClient::new(),
            wallet_address,
            private_key_bytes,
            base_url: base_url.to_string(),
            is_mainnet,
            nonce_counter: AtomicU64::new(chrono::Utc::now().timestamp_millis() as u64),
        })
    }

    fn next_nonce(&self) -> u64 {
        self.nonce_counter.fetch_add(1, Ordering::SeqCst)
    }

    fn sign_action(&self, action: &serde_json::Value) -> Result<String> {
        // 1. JSON canónico
        let action_map = action.as_object().ok_or_else(|| anyhow!("Action no es un objeto"))?;
        let sorted: BTreeMap<_, _> = action_map.iter().collect();
        let canonical_json = serde_json::to_string(&sorted)?;

        // 2. Prefijo Ethereum
        let prefix = format!("\x19Ethereum Signed Message:\n{}", canonical_json.len());
        let mut message = prefix.as_bytes().to_vec();
        message.extend(canonical_json.as_bytes());

        // 3. Hash Keccak256
        let mut hasher = Keccak256::new();
        hasher.update(&message);
        let hash = hasher.finalize();

        // 4. Mensaje para secp256k1
        let msg = Message::from_digest_slice(&hash)
            .map_err(|e| anyhow!("Hash inválido: {}", e))?;

        // 5. Clave privada
        let secret_key = SecretKey::from_slice(&self.private_key_bytes)
            .map_err(|e| anyhow!("Clave privada inválida: {}", e))?;

        // 6. Firmar
        let secp = Secp256k1::new();
        let recoverable_sig = secp.sign_ecdsa_recoverable(&msg, &secret_key);

        // 7. Serializar
        let (rec_id, sig_bytes) = recoverable_sig.serialize_compact();
        let r = &sig_bytes[0..32];
        let s = &sig_bytes[32..64];
        let v = rec_id.to_i32() as u8 + 27;

        // 8. Hex
        let mut sig_hex = hex::encode(r);
        sig_hex.push_str(&hex::encode(s));
        sig_hex.push_str(&hex::encode([v]));
        Ok(sig_hex)
    }

    pub async fn get_balance(&self, asset: &str) -> Result<Decimal> {
        let url = format!("{}/info", self.base_url);
        let payload = json!({
            "type": "clearinghouseState",
            "user": self.wallet_address
        });
        let response = self.http_client.post(&url).json(&payload).send().await?;
        let data: serde_json::Value = response.json().await?;
        if let Some(assets) = data["assetPositions"].as_array() {
            for asset_pos in assets {
                if let Some(position) = asset_pos["position"].as_object() {
                    if let Some(coin) = position.get("coin") {
                        if coin.as_str() == Some(asset) {
                            if let Some(szi) = position.get("szi") {
                                if let Some(szi_str) = szi.as_str() {
                                    if let Ok(balance_f64) = szi_str.parse::<f64>() {
                                        return Ok(Decimal::from_f64(balance_f64).unwrap_or(Decimal::ZERO));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        Ok(Decimal::ZERO)
    }

    pub async fn place_limit_order(
        &self,
        symbol: &str,
        side: HyperliquidOrderSide,
        quantity: Decimal,
        price: Decimal,
    ) -> Result<HyperliquidOrderResponse> {
        let network_name = if self.is_mainnet { "MAINNET" } else { "TESTNET" };
        let side_name = if side == HyperliquidOrderSide::Buy { "COMPRA" } else { "VENTA" };
        info!("📝 Colocando orden REAL en Hyperliquid {}: {} {} @ ${:.2}", 
              network_name, side_name, quantity, price);

        let quantity_f64 = quantity.to_f64().unwrap_or(0.0);
        let price_f64 = price.to_f64().unwrap_or(0.0);
        let url = format!("{}/exchange", self.base_url);

        let order = json!({
            "asset": symbol,
            "isBuy": side == HyperliquidOrderSide::Buy,
            "reduceOnly": false,
            "limitPx": price_f64,
            "sz": quantity_f64,
            "orderType": { "limit": { "tif": "Gtc" } }
        });

        let action = json!({
            "type": "order",
            "orders": [order],
            "grouping": "na"
        });

        let nonce = self.next_nonce();
        let signature = self.sign_action(&action)?;

        let payload = json!({
            "action": action,
            "nonce": nonce,
            "signature": signature,
            "vaultAddress": null,
            "user": self.wallet_address
        });

        info!("📤 Enviando orden firmada (nonce={})...", nonce);
        let response = self.http_client.post(&url).json(&payload).send().await?;
        let status = response.status();
        let response_text = response.text().await?;

        info!("📥 Respuesta HTTP {}: {}", status, response_text);

        if status.is_success() {
            if let Ok(json_res) = serde_json::from_str::<serde_json::Value>(&response_text) {
                if let Some(statuses) = json_res.get("statuses").and_then(|s| s.as_array()) {
                    if let Some(first_status) = statuses.first() {
                        if let Some(oid) = first_status
                            .get("resting")
                            .and_then(|r| r.get("oid"))
                            .or_else(|| first_status.get("filled").and_then(|f| f.get("oid")))
                            .and_then(|o| o.as_u64())
                        {
                            info!("✅ Orden colocada. Order ID: {}", oid);
                            return Ok(HyperliquidOrderResponse {
                                success: true,
                                order_id: oid.to_string(),
                                message: "Orden colocada".to_string(),
                            });
                        }
                        if let Some(error_msg) = first_status.get("error").and_then(|e| e.as_str()) {
                            error!("❌ Orden rechazada: {}", error_msg);
                            return Err(anyhow!("Orden rechazada: {}", error_msg));
                        }
                    }
                }
                warn!("Respuesta inesperada: {}", response_text);
                Ok(HyperliquidOrderResponse {
                    success: true,
                    order_id: format!("unknown_{}", chrono::Utc::now().timestamp()),
                    message: "Orden aceptada (ID no disponible)".to_string(),
                })
            } else {
                error!("No se pudo parsear JSON: {}", response_text);
                Err(anyhow!("Respuesta inválida del exchange"))
            }
        } else {
            error!("❌ Error HTTP: {}", response_text);
            Err(anyhow!("Error colocando orden: {}", response_text))
        }
    }

    pub async fn get_account_state(&self) -> Result<HyperliquidAccountState> {
        let url = format!("{}/info", self.base_url);
        let payload = json!({
            "type": "clearinghouseState",
            "user": self.wallet_address
        });
        let response = self.http_client.post(&url).json(&payload).send().await?;
        let data: serde_json::Value = response.json().await?;
        let mut balances = HashMap::new();
        if let Some(assets) = data["assetPositions"].as_array() {
            for asset_pos in assets {
                if let Some(position) = asset_pos["position"].as_object() {
                    if let (Some(coin), Some(szi)) = (position.get("coin"), position.get("szi")) {
                        if let (Some(coin_str), Some(szi_str)) = (coin.as_str(), szi.as_str()) {
                            if let Ok(balance_f64) = szi_str.parse::<f64>() {
                                balances.insert(coin_str.to_string(), Decimal::from_f64(balance_f64).unwrap_or(Decimal::ZERO));
                            }
                        }
                    }
                }
            }
        }
        Ok(HyperliquidAccountState {
            balances,
            wallet_address: self.wallet_address.clone(),
        })
    }
}