use crate::dex::raydium::layout::MarketStateLayoutV3;
use std::sync::{Arc, Mutex};
use borsh::BorshDeserialize;
use log::{debug, error, info};
use serde_json::{json, Value};
use solana_client::rpc_client::RpcClient;
use solana_program::pubkey::Pubkey;
use solana_sdk::genesis_config::ClusterType;
use tungstenite::Message;
use url::Url;
use crate::dex;
use crate::dex::raydium::layout::LiquidityStateLayoutV4;
use crate::dex::raydium::pool::LiquidityPoolInfo;
use crate::spl::token::WalletInformation;

pub type PoolDataSync = Arc<Mutex<PoolChunk>>;

pub struct PoolChunk {
    pub liquidity_state: Option<LiquidityStateLayoutV4>,
    pub market_state: Option<MarketStateLayoutV3>,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct WebSocketClient {
    wss_url: String,
    http_url: String,
}

#[derive(Debug, Clone)]
pub struct TaskConfig {
    pub sell_percent: f64,
    pub sell_interval: f64,
    pub rpc_url: String,
    pub buy_amount: f64,
}

impl WebSocketClient {
    pub fn new(wss_url: &str, http_url: &str) -> WebSocketClient {
        WebSocketClient {
            wss_url: wss_url.to_string(),
            http_url: http_url.to_string(),
        }
    }

    pub fn wait_for_pool(pool_data_sync: PoolDataSync, ws: WebSocketClient, base_mint: &Pubkey, quote_mint: &Pubkey, cluster_type: ClusterType) {
        let base_mint = base_mint.clone();
        let quote_mint = quote_mint.clone();

        let db_1 = pool_data_sync.clone();
        let ws_1 = ws.clone();
        let cluster_1 = cluster_type.clone();

        tokio::spawn(async move {
            ws_1.wss_get_market(&base_mint, &quote_mint, cluster_1, db_1).await;
        });

        let db_2 = pool_data_sync.clone();
        let ws_2 = ws.clone();
        let cluster_2 = cluster_type.clone();

        tokio::spawn(async move {
            ws_2.wss_get_liquidity(&base_mint, &quote_mint, cluster_2, db_2).await;
        });
    }

    pub async fn wss_get_market(&self, base_mint: &Pubkey, quote_mint: &Pubkey,
                                cluster_type: ClusterType,
                                pool_data_sync: PoolDataSync) {
        match cluster_type {
            ClusterType::Devnet => {
                self.wss_get_market_with_program_id(base_mint, quote_mint, pool_data_sync, dex::openbook::SERUM_PROGRAM_DEV_ID).await;
            }
            ClusterType::MainnetBeta => {
                self.wss_get_market_with_program_id(base_mint, quote_mint, pool_data_sync, dex::openbook::SERUM_PROGRAM_ID).await;
            }
            _ => {
                unimplemented!()
            }
        }
    }

    pub async fn wss_get_market_with_program_id<U: ToString + serde::Serialize>(&self, base_mint: &Pubkey, quote_mint: &Pubkey,
                                                                                pool_data_sync: PoolDataSync, program_id: U) {
        let url = Url::parse(&self.wss_url).unwrap();
        let (mut socket, _response) = tungstenite::connect(url).unwrap();
        info!("MarketStateLayoutV3: Connected to the server");

        socket.send(
            Message::Binary(
                serde_json::to_vec(
                    &json!(
                   {
                       "jsonrpc": "2.0",
                       "id": 1,
                       "method": "programSubscribe",
                       "params": json!([program_id, &MarketStateLayoutV3::get_config(base_mint, quote_mint)])
                   }
               )
                ).unwrap()
            )
        ).unwrap();

        loop {
            match socket.read() {
                Ok(e) => {
                    info!("Subscribed or received data");
                    debug!("wss_get_market_with_program_id: {:?}", e);

                    let d = Self::parse_wss_data(e);
                    if d.is_some() {
                        let mut pool_data = pool_data_sync.lock().unwrap();
                        pool_data.market_state = Some(dex::raydium::layout::MarketStateLayoutV3::try_from_slice(&d.unwrap()).unwrap());
                        break;
                    }
                }
                Err(e) => {
                    error!("{:?}", e);
                    break;
                }
            }
        }
    }

    #[allow(deprecated)]
    pub fn get_account_data(account: &Value) -> Result<Vec<u8>, dex::raydium::error::ParserError> {
        let account_data_vector = match account.get("data") {
            None => return Err(dex::raydium::error::ParserError::AccountDataNotFound),
            Some(a) => {
                a.as_array()
                 .unwrap()
                 .to_vec()
            }
        };

        let account_data = match account_data_vector.first() {
            None => return Err(dex::raydium::error::ParserError::AccountDataNotFound),
            Some(a) => {
                a.as_str().unwrap()
            }
        };

        return match base64::decode(account_data) {
            Ok(d) => Ok(d),
            Err(e) => Err(dex::raydium::error::ParserError::AccountDataDecodeError(e.to_string()))
        };
    }
    pub async fn wss_get_liquidity(&self, base_mint: &Pubkey,
                                   quote_mint: &Pubkey,
                                   cluster_type: ClusterType,
                                   pool_data_sync: PoolDataSync) {
        match cluster_type {
            ClusterType::Devnet => {
                self.wss_get_liquidity_with_program_id(base_mint, quote_mint, pool_data_sync, dex::raydium::AMM_PROGRAM_DEV_ID).await;
            }
            ClusterType::MainnetBeta => {
                self.wss_get_liquidity_with_program_id(base_mint, quote_mint, pool_data_sync, dex::raydium::AMM_PROGRAM_ID).await;
            }
            _ => {
                unimplemented!()
            }
        }
    }

    pub async fn wss_get_liquidity_with_program_id<U: ToString + serde::Serialize>(&self, base_mint: &Pubkey, quote_mint: &Pubkey,
                                                                                   pool_data_sync: PoolDataSync, program_id: U) {
        let url = Url::parse(&self.wss_url).unwrap();
        let (mut socket, _response) = tungstenite::connect(url).unwrap();
        info!("LiquidityStateLayoutV4: Connected to the server");

        socket.send(
            Message::Binary(
                serde_json::to_vec(
                    &json!(
                    {
                        "jsonrpc": "2.0",
                        "id": 1,
                        "method": "programSubscribe",
                        "params": json!([program_id, &LiquidityStateLayoutV4::get_config(base_mint, quote_mint)])
                    }
                )
                ).unwrap()
            )
        ).unwrap();

        loop {
            match socket.read() {
                Ok(e) => {
                    info!("Subscribed or received data");
                    debug!("wss_get_liquidity_with_program_id: {:?}", e);

                    let d = Self::parse_wss_data(e);
                    if d.is_some() {
                        let mut pool_data = pool_data_sync.lock().unwrap();
                        pool_data.liquidity_state = Some(LiquidityStateLayoutV4::try_from_slice(&d.unwrap()).unwrap());
                        break;
                    }
                }
                Err(e) => {
                    error!("{:?}", e);
                    break;
                }
            }
        }
    }

    fn parse_wss_data(msg: Message) -> Option<Vec<u8>> {
        let result: Value = serde_json::from_str(msg.to_text().unwrap()).unwrap();

        return match result.get("method") {
            None => {
                None
            },
            Some(_) => {
                let params = result.get("params").unwrap();
                let result = params.get("result").unwrap();
                let value = result.get("value").unwrap();
                let account = value.get("account").unwrap();
                Some(Self::get_account_data(account).unwrap())
            }
        }
    }

    pub async fn run_task(f: impl Fn(Vec<WalletInformation>, &TaskConfig, &LiquidityPoolInfo, ClusterType),
                          args: Vec<WalletInformation>,
                          task_config: TaskConfig,
                          cluster_type: ClusterType,
                          pool_data_sync: PoolDataSync) {
        loop {
            let pool_data = pool_data_sync.lock().unwrap();
            if pool_data.liquidity_state.is_some() && pool_data.market_state.is_some() {
                let pool_info =
                    LiquidityPoolInfo::build(pool_data.liquidity_state.unwrap(), pool_data.market_state.unwrap(), cluster_type)
                        .expect("failed building liquidity pool info");
                f(args, &task_config, &pool_info.0, cluster_type);
                break;
            }
        }
    }
}