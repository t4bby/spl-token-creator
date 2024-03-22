use crate::dex::raydium::layout::MarketStateLayoutV3;
use std::sync::{Arc, Mutex};
use std::time::UNIX_EPOCH;
use borsh::BorshDeserialize;
use chrono::DateTime;
use colored::Colorize;
use log::{debug, error, info};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use solana_program::native_token::lamports_to_sol;
use solana_program::pubkey::Pubkey;
use solana_sdk::genesis_config::ClusterType;
use tungstenite::Message;
use url::Url;
use crate::dex;
use crate::dex::raydium;
use crate::dex::raydium::layout::LiquidityStateLayoutV4;
use crate::dex::raydium::pool::LiquidityPoolInfo;
use crate::spl::token::WalletInformation;

pub type PoolDataSync = Arc<Mutex<PoolChunk>>;

pub struct PoolChunk {
    pub liquidity_state: Option<LiquidityStateLayoutV4>,
    pub market_state: Option<MarketStateLayoutV3>,
    pub liquidity_amount: Option<u64>,
    pub task_done: bool,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct WebSocketClient {
    wss_url: String,
}

#[derive(Debug, Clone)]
pub struct TaskConfig {
    pub sell_percent: f64,
    pub sell_interval: f64,
    pub rpc_url: String,
    pub buy_amount: f64,
    pub overhead: f64,
}

#[derive(Debug, Clone)]
pub struct LiquidityTaskConfig {
    pub rpc_url: String,
    pub target_liquidity: f64,
    pub initial_liquidity: f64,
}


#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UiTokenAmmount {
    pub amount: String,
    pub decimals: u8,
    pub ui_amount: f64,
    pub ui_amount_string: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TokenBalance {
    pub account_index: u64,
    pub mint: String,
    pub owner: String,
    pub program_id: String,
    pub ui_token_amount: UiTokenAmmount,
}

impl TokenBalance {
    pub fn parse(value: &Value) -> Self {
        let account_index = value.get("accountIndex").unwrap().as_u64().unwrap();
        let mint = value.get("mint").unwrap().as_str().unwrap();
        let owner = value.get("owner").unwrap().as_str().unwrap();
        let program_id = value.get("programId").unwrap().as_str().unwrap();
        let ui_token_amount = value.get("uiTokenAmount").unwrap();
        let amount = ui_token_amount.get("amount").unwrap().as_str().unwrap();
        let decimals = ui_token_amount.get("decimals").unwrap().as_u64().unwrap() as u8;
        let ui_amount = match ui_token_amount.get("uiAmount") {
            None => 0f64,
            Some(a) => {
                if a.is_null() {
                    0f64
                } else {
                    a.as_f64().unwrap()
                }
            }
        };
        let ui_amount_string = ui_token_amount.get("uiAmountString").unwrap().as_str().unwrap();

        TokenBalance {
            account_index,
            mint: mint.to_string(),
            owner: owner.to_string(),
            program_id: program_id.to_string(),
            ui_token_amount: UiTokenAmmount {
                amount: amount.to_string(),
                decimals,
                ui_amount,
                ui_amount_string: ui_amount_string.to_string(),
            },
        }
    }
}

impl WebSocketClient {
    pub fn new<U: ToString>(wss_url: U) -> WebSocketClient {
        WebSocketClient {
            wss_url: wss_url.to_string(),
        }
    }

    pub fn wss_monitor_liquidity_changes(&self, address: &str) {
        let url = Url::parse(&self.wss_url).unwrap();
        let (mut socket, _response) = tungstenite::connect(url).unwrap();
        info!("monitor_account: Connected to the server");

        let params = json!({
        "encoding": "base64",
        "commitment": "confirmed",
        "transactionDetails": "accounts",
        "maxSupportedTransactionVersion": 0,
        "showRewards": false
        });

        let account_param = json!({
         "mentionsAccountOrProgram": address
        });

        socket.send(
            Message::Binary(
                serde_json::to_vec(
                    &json!(
                   {
                       "jsonrpc": "2.0",
                       "id": 1,
                       "method": "blockSubscribe",
                       "params": json!([account_param, params])
                   }
               )
                ).unwrap()
            )
        ).unwrap();


        let mut subscribed = false;
        let mut subscription_id;
        loop {
            match socket.read() {
                Ok(e) => {
                    debug!("{:?}", e.to_text().unwrap());
                    let parsed: Value = match serde_json::from_str(e.to_text().unwrap()) {
                        Ok(a) => a,
                        Err(_) => {
                            continue;
                        }
                    };

                    match parsed.get("result") {
                        None => {}
                        Some(_) => {
                            subscription_id = parsed.get("result").unwrap().as_u64().unwrap();
                            info!("[monitor_account] Subscription ID: {}", subscription_id);
                            subscribed = true;
                        }
                    }

                    if subscribed == false {
                        info!("[monitor_account] Subscription failed");
                        break;
                    }

                    let liquidity_data = Self::parse_monitor_data(&parsed, true);
                    if liquidity_data.is_some() {
                        info!("Liquidity: {} SOL", lamports_to_sol(liquidity_data.unwrap()));
                    }
                }
                Err(e) => {
                    error!("{:?}", e);
                    break;
                }
            }
        }
    }

    pub async fn wss_get_liquidity_data(&self, address: &Pubkey, pool_data_sync: PoolDataSync) {
        let url = Url::parse(&self.wss_url).unwrap();
        let (mut socket, _response) = tungstenite::connect(url).unwrap();
        info!("Liquidity Monitor: Connected to the server");

        let params = json!({
        "encoding": "base64",
        "commitment": "confirmed",
        "transactionDetails": "accounts",
        "maxSupportedTransactionVersion": 0,
        "showRewards": false
        });

        let account_param = json!({
         "mentionsAccountOrProgram": address.to_string()
        });

        socket.send(
            Message::Binary(
                serde_json::to_vec(
                    &json!(
                   {
                       "jsonrpc": "2.0",
                       "id": 1,
                       "method": "blockSubscribe",
                       "params": json!([account_param, params])
                   }
               )
                ).unwrap()
            )
        ).unwrap();


        let mut subscribed = false;
        let mut subscription_id;
        loop {
            match socket.read() {
                Ok(e) => {
                    let parsed: Value = match serde_json::from_str(e.to_text().unwrap()) {
                        Ok(a) => a,
                        Err(_) => {
                            continue;
                        }
                    };

                    match parsed.get("result") {
                        None => {}
                        Some(_) => {
                            subscription_id = parsed.get("result").unwrap().as_u64().unwrap();
                            info!("[Liquidity Monitor] Subscription ID: {}", subscription_id);
                            subscribed = true;
                        }
                    }

                    if subscribed == false {
                        info!("[Liquidity Monitor] Subscription failed");
                        break;
                    }

                    let price_data = Self::parse_monitor_data(&parsed, false);
                    let mut pool_data = pool_data_sync.lock().unwrap();
                    if price_data.is_some() {
                        pool_data.liquidity_amount = Some(price_data.unwrap());
                    }
                    if pool_data.task_done {
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

    fn parse_monitor_data(value: &Value, show_total_liquidity: bool) -> Option<u64> {
        match value.get("params") {
            None => None,
            Some(a) => {
                let result = a.get("result").unwrap();
                let value = result.get("value").unwrap();
                let block = value.get("block").unwrap();
                let transactions = block.get("transactions")
                                        .unwrap().as_array().unwrap();

                let mut price_data: Option<u64> = None;

                for transaction in transactions.iter() {
                    let meta = transaction.get("meta").unwrap();
                    let err = meta.get("err").unwrap();
                    if err.is_null() == false {
                        continue;
                    }
                    //std::fs::write("transaction_block.json", a.to_string()).unwrap();

                    let pre_balances = meta.get("preTokenBalances")
                                           .unwrap()
                                           .as_array()
                                           .unwrap();

                    let mut pre_token_balance: Vec<TokenBalance> = vec![];
                    let mut post_token_balance: Vec<TokenBalance> = vec![];
                    for balance in pre_balances.iter() {
                        pre_token_balance.push(TokenBalance::parse(&balance));
                    }

                    let post_balances = meta.get("postTokenBalances")
                                            .unwrap()
                                            .as_array().unwrap();
                    for balance in post_balances.iter() {
                        post_token_balance.push(TokenBalance::parse(&balance));
                    }

                    let transaction_info = transaction.get("transaction").unwrap();
                    let account_keys = transaction_info.get("accountKeys").unwrap().as_array().unwrap();

                    let mut is_raydium_program_transaction = false;
                    for key in account_keys.iter() {
                        let pub_key = key.get("pubkey").unwrap().as_str().unwrap();
                        if pub_key == raydium::AMM_PROGRAM_ID || pub_key == raydium::AMM_PROGRAM_DEV_ID {
                            is_raydium_program_transaction = true;
                            break;
                        }
                    }

                    if is_raydium_program_transaction == false {
                        continue;
                    }

                    for token in pre_token_balance.iter() {
                        for post_token in post_token_balance.iter() {
                            if (token.owner == raydium::AUTHORITY_ID && post_token.owner == raydium::AUTHORITY_ID) ||
                                (token.owner == raydium::AUTHORITY_DEV_ID && post_token.owner == raydium::AUTHORITY_DEV_ID) {
                                if token.mint == spl_token::native_mint::id().to_string()
                                    && post_token.mint == spl_token::native_mint::id().to_string() {
                                    debug!("Pre Token: {}", token.mint);
                                    debug!("Pre Address: {}", token.owner);

                                    debug!("Post Token: {}", post_token.mint);
                                    debug!("Post Address: {}", post_token.owner);

                                    let pre_amount: u64 = token.ui_token_amount.amount.parse().unwrap();
                                    debug!("Pre Amount: {}", lamports_to_sol(pre_amount).to_string().red());

                                    let post_amount: u64 = post_token.ui_token_amount.amount.parse().unwrap();
                                    debug!("Post Amount: {}", lamports_to_sol(post_amount).to_string().green());
                                    price_data = Some(post_amount);

                                    let balance = post_amount as i64 - pre_amount as i64;
                                    if balance > 0 {
                                        info!("[BUY] Liquidity: +{} SOL", lamports_to_sol(balance.abs() as u64).to_string().green());
                                        break;
                                    }
                                    info!("[SELL] Liquidity: -{} SOL", lamports_to_sol(balance.abs() as u64).to_string().red());
                                    break;
                                }
                            }
                        }
                    }

                    let sigs = transaction_info.get("signatures").unwrap().as_array().unwrap();
                    for sig in sigs.iter() {
                        info!("Signature: {}", sig.as_str().unwrap().bright_cyan());
                    }
                }

                if price_data.is_some() {
                    if show_total_liquidity {
                        info!("Total Liquidity: {} SOL",
                            lamports_to_sol(price_data.unwrap()).to_string().green());
                    }
                    Some(price_data.unwrap())
                } else {
                    None
                }
            }
        }
    }

    pub fn wait_for_pool(pool_data_sync: PoolDataSync, ws: &WebSocketClient, base_mint: &Pubkey, quote_mint: &Pubkey, cluster_type: ClusterType) {
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

    pub fn wait_for_liquidity_pool(pool_data_sync: PoolDataSync, ws: &WebSocketClient, base_mint: &Pubkey, quote_mint: &Pubkey, cluster_type: ClusterType) {
        let base_mint = base_mint.clone();
        let quote_mint = quote_mint.clone();

        let db_1 = pool_data_sync.clone();
        let ws_1 = ws.clone();
        let cluster_1 = cluster_type.clone();

        tokio::spawn(async move {
            ws_1.wss_get_liquidity(&base_mint, &quote_mint, cluster_1, db_1).await;
        });
    }


    pub fn monitor_liquidity(pool_data_sync: PoolDataSync,
                             wss_pool: &WebSocketClient,
                             wss_liquidity: &WebSocketClient,
                             base_mint: &Pubkey,
                             quote_mint: &Pubkey,
                             cluster_type: ClusterType,
                             market_state: Option<MarketStateLayoutV3>,
                             liquidity_state: Option<LiquidityStateLayoutV4>) {

        if market_state.is_some() && liquidity_state.is_some() {
            info!("Market and Liquidity found");
            let mut pool_data = pool_data_sync.lock().unwrap();
            pool_data.liquidity_state = Some(liquidity_state.unwrap());
            pool_data.market_state = Some(market_state.unwrap());
        } else if market_state.is_some() && liquidity_state.is_none() {
            info!("Market found, waiting for liquidity pool");
            let mut pool_data = pool_data_sync.lock().unwrap();
            pool_data.market_state = Some(market_state.unwrap());
            Self::wait_for_liquidity_pool(pool_data_sync.clone(), &wss_pool, base_mint, quote_mint, cluster_type)
        } else {
            info!("Market and Liquidity not found. Waiting for market and pool");
            Self::wait_for_pool(pool_data_sync.clone(), &wss_pool, base_mint, quote_mint, cluster_type);
        }

        let base_mint = base_mint.clone();
        let db_1 = pool_data_sync.clone();
        let ws_1 = wss_liquidity.clone();

        tokio::spawn(async move {
            ws_1.wss_get_liquidity_data(&base_mint, db_1).await;
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

        let mut subscribed = false;
        let mut subscription_id;
        loop {
            match socket.read() {
                Ok(e) => {
                    let parsed: Value = match serde_json::from_str(e.to_text().unwrap()) {
                        Ok(a) => a,
                        Err(_) => {
                            continue;
                        }
                    };

                    match parsed.get("result") {
                        None => {}
                        Some(_) => {
                            subscription_id = parsed.get("result").unwrap().as_u64().unwrap();
                            info!("[MarketStateLayoutV3] Subscription ID: {}", subscription_id);
                            subscribed = true;
                        }
                    }

                    if subscribed == false {
                        info!("[MarketStateLayoutV3] Subscription failed");
                        break;
                    }

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

        let mut subscribed = false;
        let mut subscription_id;
        loop {
            match socket.read() {
                Ok(e) => {
                    let parsed: Value = match serde_json::from_str(e.to_text().unwrap()) {
                        Ok(a) => a,
                        Err(_) => {
                            continue;
                        }
                    };

                    match parsed.get("result") {
                        None => {}
                        Some(_) => {
                            subscription_id = parsed.get("result").unwrap().as_u64().unwrap();
                            info!("[LiquidityStateLayoutV4] Subscription ID: {}", subscription_id);
                            subscribed = true;
                        }
                    }

                    if subscribed == false {
                        info!("[LiquidityStateLayoutV4] Subscription failed");
                        break;
                    }

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

    #[allow(deprecated)]
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

                let naive = chrono::NaiveDateTime::from_timestamp(pool_info.liquidity_state.pool_open_time as i64, 0);
                let datetime: DateTime<chrono::Utc> = DateTime::from_utc(naive, chrono::Utc);
                let new_date = datetime.format("%Y-%m-%d %H:%M:%S %Z");
                info!("Pool Open Time: {}", new_date);

                loop {
                    let now = std::time::SystemTime::now();
                    let since_epoch = now.duration_since(UNIX_EPOCH).expect("Time went backwards");
                    let seconds = since_epoch.as_secs();
                    if pool_info.liquidity_state.pool_open_time <= seconds {
                        info!("Pool Opened");
                        f(args, &task_config, &pool_info, cluster_type);
                        break;
                    }
                }
                break;
            }
        }
    }
    #[allow(deprecated)]
    pub async fn run_liquidity_change_task(f: impl Fn(WalletInformation, &LiquidityTaskConfig, &LiquidityPoolInfo, ClusterType),
                                           args: WalletInformation,
                                           task_config: LiquidityTaskConfig,
                                           cluster_type: ClusterType,
                                           pool_data_sync: PoolDataSync) {
        info!("Waiting for liquidity pool to be created");
        loop {
            let pool_data = pool_data_sync.lock().unwrap();
            if pool_data.liquidity_state.is_some() && pool_data.market_state.is_some() {
                info!("Liquidity Pool Created");
                let pool_info =
                    LiquidityPoolInfo::build(pool_data.liquidity_state.unwrap(), pool_data.market_state.unwrap(), cluster_type)
                        .expect("failed building liquidity pool info");

                let naive = chrono::NaiveDateTime::from_timestamp(pool_info.liquidity_state.pool_open_time as i64, 0);
                let datetime: DateTime<chrono::Utc> = DateTime::from_utc(naive, chrono::Utc);
                let new_date = datetime.format("%Y-%m-%d %H:%M:%S %Z");
                info!("Pool Open Time: {}", new_date);

                let target_liquidity: f64 = task_config.target_liquidity;
                info!("Target Liquidity: {} SOL", target_liquidity);

                loop {
                    let now = std::time::SystemTime::now();
                    let since_epoch = now.duration_since(UNIX_EPOCH).expect("Time went backwards");
                    let seconds = since_epoch.as_secs();
                    if pool_info.liquidity_state.pool_open_time <= seconds {
                        info!("Pool Opened");
                        break;
                    }
                }

                drop(pool_data);
                info!("Checking for liquidity increase/decrease");
                let mut current_liquidity  = task_config.initial_liquidity;
                loop {
                    let mut pool_data = pool_data_sync.lock().unwrap();
                    if pool_data.liquidity_amount.is_some() {
                        let temp_liquidity = lamports_to_sol(pool_data.liquidity_amount.unwrap());
                        // check if liquidity is the same
                        if current_liquidity == temp_liquidity {
                            continue;
                        }
                        // update current liquidity
                        current_liquidity = temp_liquidity;
                        if current_liquidity > task_config.initial_liquidity {
                            info!("Liquidity: {} SOL", current_liquidity.to_string().green());
                        }
                        if current_liquidity < task_config.initial_liquidity {
                            info!("Liquidity: {} SOL", current_liquidity.to_string().red());
                        }
                        if current_liquidity >= target_liquidity {
                            pool_data.task_done = true;
                            drop(pool_data);
                            info!("Target Liquidity Reached");
                            info!("Liquidity: {} SOL", current_liquidity.to_string().green());
                            f(args.clone(), &task_config, &pool_info, cluster_type);
                            break;
                        }
                    }
                }
                break;
            }
        }
    }
}