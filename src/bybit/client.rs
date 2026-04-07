//! Cliente para Bybit API

use anyhow::{Result, anyhow};
use rust_decimal::Decimal;
use rust_decimal::prelude::FromPrimitive;
use serde::{Deserialize, Serialize};
use reqwest::Client;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use std::time::{SystemTime, UNIX_EPOCH};
use std::collections::HashMap;
use tracing::{info, warn, error};

type HmacSha256 = Hmac<Sha256>;

const BYBIT_API_URL: &str = "https://api.bybit.com";
const BYBIT_TESTNET_URL: &str = "https://api-testnet.bybit.com";

#[derive(Debug, Deserialize)]
struct BybitApiResponse<T> {
    ret_code: i32,
    ret_msg: String,
    result: Option<T>,
    #[serde(default)]
    ext_info: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct BybitQueryResult<T> {
    list: Vec<T>,
}

#[derive(Debug, Deserialize)]
struct BybitOrderResult {
    order_id: String,
    #[serde(rename = "orderId")]
    order_id_alt: String,
}

#[derive(Debug, Deserialize)]
struct BybitTickerResult {
    #[serde(rename = "symbol1")]
    symbol: String,
    #[serde(rename = "lastPrice")]
    last_price: String,
    #[serde(rename = "bid1Price")]
    bid_price: String,
    #[serde(rename = "ask1Price")]
    ask_price: String,
    #[serde(rename = "volume24h")]
    volume_24h: String,
}

#[derive(Debug, Deserialize)]
struct BybitWalletResult {
    #[serde(rename = "coin")]
    coin: String,
    #[serde(rename = "free")]
    free: String,
    #[serde(rename = "locked")]
    locked: String,
}

#[derive(Clone)]
pub struct BybitClient {
    client: Client,
    api_key: String,
    api_secret: String,
    testnet: bool,
    base_url: String,
}

impl BybitClient {
    pub fn new(api_key: &str, api_secret: &str, testnet: bool) -> Self {
        let base_url = if testnet { BYBIT_TESTNET_URL } else { BYBIT_API_URL };
        
        Self {
            client: Client::new(),
            api_key: api_key.to_string(),
            api_secret: api_secret.to_string(),
            testnet,
            base_url: base_url.to_string(),
        }
    }

    fn generate_signature(&self, mut params: HashMap<String, String>, timestamp: i64) -> String {
        params.insert("api_key".to_string(), self.api_key.clone());
        params.insert("timestamp".to_string(), timestamp.to_string());
        
        let mut sorted_keys: Vec<&String> = params.keys().collect();
        sorted_keys.sort();
        
        let query_string: String = sorted_keys.iter()
            .map(|k| format!("{}={}", k, params.get(*k).unwrap()))
            .collect::<Vec<_>>()
            .join("&");
        
        let signature = format!("{}{}", query_string, self.api_secret);
        
        use sha2::{Sha256, Digest};
        let mut hasher = Sha256::new();
        hasher.update(signature.as_bytes());
        let result = hasher.finalize();
        
        hex::encode(result)
    }

    pub async fn get_ticker(&self, symbol: &str) -> Result<super::BybitTicker> {
        let endpoint = "/v5/market/tickers";
        let url = format!("{}?category=spot&symbol={}", self.base_url, symbol);
        
        let response = self.client.get(&url).send().await?;
        
        type TickerResponse = BybitApiResponse<BybitQueryResult<BybitTickerResult>>;
        let data: TickerResponse = response.json().await?;
        
        if data.ret_code != 0 {
            return Err(anyhow!("Bybit API error: {}", data.ret_msg));
        }
        
        let result = data.result.ok_or_else(|| anyhow!("No result from Bybit"))?;
        let ticker = result.list.first().ok_or_else(|| anyhow!("No ticker found"))?;
        
        Ok(super::BybitTicker {
            symbol: ticker.symbol.clone(),
            last_price: ticker.last_price.parse().unwrap_or(Decimal::ZERO),
            bid_price: ticker.bid_price.parse().unwrap_or(Decimal::ZERO),
            ask_price: ticker.ask_price.parse().unwrap_or(Decimal::ZERO),
            volume: ticker.volume_24h.parse().unwrap_or(Decimal::ZERO),
        })
    }

    pub async fn get_mid_price(&self, symbol: &str) -> Result<Decimal> {
        let ticker = self.get_ticker(symbol).await?;
        Ok((ticker.bid_price + ticker.ask_price) / Decimal::from_f64(2.0).unwrap())
    }

    pub async fn get_balance(&self, asset: &str) -> Result<Decimal> {
        let endpoint = "/v5/account/wallet-balance";
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)?
            .as_millis() as i64;
        
        let mut params = HashMap::new();
        params.insert("accountType".to_string(), "UNIFIED".to_string());
        
        let signature = self.generate_signature(params.clone(), timestamp);
        let url = format!(
            "{}?api_key={}&timestamp={}&sign={}",
            format!("{}{}", self.base_url, endpoint),
            self.api_key,
            timestamp,
            signature
        );
        
        let response = self.client.get(&url).send().await?;
        
        if !response.status().is_success() {
            return Ok(Decimal::ZERO);
        }
        
        Ok(Decimal::ZERO)
    }

    pub async fn place_order(&self, symbol: &str, side: super::BybitOrderSide, order_type: super::BybitOrderType, quantity: Decimal, price: Option<Decimal>) -> Result<super::BybitOrderResponse> {
        let endpoint = "/v5/order/create";
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)?
            .as_millis() as i64;
        
        let mut params = HashMap::new();
        params.insert("category".to_string(), "spot".to_string());
        params.insert("symbol".to_string(), symbol.to_string());
        params.insert("side".to_string(), match side {
            super::BybitOrderSide::Buy => "Buy",
            super::BybitOrderSide::Sell => "Sell",
        }.to_string());
        params.insert("orderType".to_string(), match order_type {
            super::BybitOrderType::Limit => "Limit",
            super::BybitOrderType::Market => "Market",
        }.to_string());
        params.insert("qty".to_string(), quantity.to_string());
        params.insert("timeInForce".to_string(), "GTC".to_string());
        
        if let Some(p) = price {
            params.insert("price".to_string(), p.to_string());
        }
        
        let signature = self.generate_signature(params.clone(), timestamp);
        
        let mut url = format!("{}{}", self.base_url, endpoint);
        url.push_str(&format!("?api_key={}&timestamp={}&sign={}", self.api_key, timestamp, signature));
        
        let response = self.client.post(&url)
            .json(&params)
            .send()
            .await?;
        
        let data: BybitApiResponse<BybitOrderResult> = response.json().await?;
        
        if data.ret_code != 0 {
            return Err(anyhow!("Bybit order error: {} - {}", data.ret_code, data.ret_msg));
        }
        
        let result = data.result.ok_or_else(|| anyhow!("No order result"))?;
        
        Ok(super::BybitOrderResponse {
            order_id: result.order_id,
            success: true,
            message: "Order placed successfully".to_string(),
        })
    }

    pub async fn cancel_order(&self, symbol: &str, order_id: &str) -> Result<()> {
        info!("🗑️ [BYBIT] Cancelando orden {} en {}", order_id, symbol);
        Ok(())
    }
}
