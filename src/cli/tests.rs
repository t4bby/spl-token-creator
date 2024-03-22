#[tokio::test(flavor = "multi_thread")]
async fn test_rug() {
    use std::io;
    use std::str::FromStr;
    use io::Write;
    use std::sync::{Arc, Mutex};
    use chrono::Local;
    use colored::Colorize;
    use env_logger::Builder;
    use solana_client::rpc_client::RpcClient;
    use solana_program::pubkey::Pubkey;
    use solana_sdk::genesis_config::ClusterType;
    use solana_sdk::signature::Keypair;
    use crate::dex::raydium::layout::{LiquidityStateLayoutV4, MarketStateLayoutV3};
    use crate::dex::raydium::pool::LiquidityPoolInfo;
    use crate::spl::token::WalletInformation;
    use crate::utils;
    use crate::utils::websocket::{LiquidityTaskConfig, WebSocketClient};
    use log::Level;
    use crate::dex::raydium;

    let mut log_builder = Builder::new();
    log_builder.format(|buf, record| {
        let level = record.level().clone();
        let record_level: String;
        match level {
            Level::Error => {
                record_level = "ERROR".red().to_string();
            },
            Level::Info => {
                record_level = "INFO".green().to_string();
            },
            _ => {
                record_level = "DEBUG".yellow().to_string();
            }
        }

        let time = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        writeln!(buf,
                 "{} > {} : {}",
                 time.blue(),
                 record_level,
                 record.args()
        )
    });

    // log_builder.filter(Some("api"), log::LevelFilter::Debug);
    // log_builder.filter(Some("spl"), log::LevelFilter::Debug);
    // log_builder.filter(Some("dex"), log::LevelFilter::Debug);
    // log_builder.filter(Some("api"), log::LevelFilter::Debug);
    // log_builder.filter(Some("utils"), log::LevelFilter::Info);
    log_builder.filter_level(log::LevelFilter::Info);

    log_builder.init();

    let keypair = Keypair::from_base58_string("51nVoW8aHGnfvjorXJPuLmAuG8mVPeb4HABrHLK9FEkvHiE8Ebuq4bi2G1zcaqbZjmyQNyFhefce5y3Qt4Wuvbr9");
    let rpc_client = RpcClient::new("https://solana-mainnet.g.alchemy.com/v2/c0XkSgm_psC_JZmca4iNH51GO9su5j9Q");
    let wss_pool_rpc_client = WebSocketClient::new("wss://rpc.ankr.com/solana/ws/56b61a7d360a9704960c1cc45fc16e5a4402a8661781c09fee3502a230fbc0d7");
    let wss_liquidity_rpc_client = WebSocketClient::new("wss://chaotic-clean-choice.solana-mainnet.quiknode.pro/3a710dcb65ef3cef9ff255c493cb27056bfeea71/");
    let cluster_type = ClusterType::MainnetBeta;

    let initial_liquidity = 14f64;
    let target_liquidity = 15f64;

    let base_mint_pub = Pubkey::from_str("26k8LBzbfTtoSkc92Ziq6eemZeB8eLQ5wrHwqrjYTFDS").unwrap();
    let quote_mint_pub = spl_token::native_mint::id();

    let pool_data_sync = Arc::new(
        Mutex::new(utils::websocket::PoolChunk {
            liquidity_state: None,
            market_state: None,
            liquidity_amount: None,
            task_done: false,
        }));

    let task_config = LiquidityTaskConfig {
        rpc_url: rpc_client.url(),
        target_liquidity,
        initial_liquidity,
    };

    // check if market already exists
    let market_state = match MarketStateLayoutV3::get_with_reqwest(
        &rpc_client.url(),
        &base_mint_pub,
        &quote_mint_pub,
        cluster_type,
    ).await {
        Ok(a) => {
            Some(a)
        }
        Err(_) => {
            None
        }
    };

    // check if pool already exists
    let liquidity_state = match LiquidityStateLayoutV4::get_with_reqwest(
        &rpc_client.url(),
        &base_mint_pub,
        &quote_mint_pub,
        cluster_type,
    ).await {
        Ok(a) => {
            Some(a)
        }
        Err(_) => {
            None
        }
    };

    WebSocketClient::monitor_liquidity(
        pool_data_sync.clone(),
        &wss_pool_rpc_client,
        &wss_liquidity_rpc_client,
        &base_mint_pub,
        &quote_mint_pub,
        cluster_type,
        market_state,
        liquidity_state
    );

    let wallet_information = WalletInformation {
        wallet: keypair.to_base58_string(),
        wsol_account: Default::default(),
        token_account: Default::default(),
        balance: 0,
        create_token_account_instruction: None,
    };

    WebSocketClient::run_liquidity_change_task(|token_creator: WalletInformation,
                                                _task_config: &LiquidityTaskConfig,
                                                liquidity_pool_info: &LiquidityPoolInfo,
                                                cluster_type: ClusterType| {
        let connection = RpcClient::new(&task_config.rpc_url);
        let payer = Keypair::from_base58_string(&token_creator.wallet);
        raydium::remove_liquidity(&connection, &payer, ".", &liquidity_pool_info, cluster_type);
    }, wallet_information, task_config.clone(), cluster_type, pool_data_sync).await;
}