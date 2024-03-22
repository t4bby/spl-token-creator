use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::time::UNIX_EPOCH;
use colored::Colorize;
use config_file::FromConfigFile;
use log::{debug, error, info};
use solana_client::rpc_client::RpcClient;
use solana_program::native_token::{lamports_to_sol, sol_to_lamports};
use solana_program::pubkey::Pubkey;
use solana_sdk::genesis_config::ClusterType;
use solana_sdk::signature::{Keypair, Signer};
use tokio::time::sleep;
use config::ProjectConfig;
use dex::raydium;
use crate::spl::token::{create_wsol_account, WalletInformation};
use crate::{api, dex, spl, utils};
use crate::cli::config::{Config, LiquidityConfig, MarketConfig};
use crate::dex::raydium::layout::{LiquidityStateLayoutV4, MarketStateLayoutV3};
use crate::dex::raydium::pool::LiquidityPoolInfo;
use crate::dex::raydium::swap;
use crate::utils::websocket::{LiquidityTaskConfig, WebSocketClient};

pub mod args;
pub mod config;
mod tests;


pub async fn create_market(
    rpc_client: &RpcClient,
    keypair: &Keypair,
    project_dir: String,
    project_config: &ProjectConfig,
    quote_mint: String,
    event_queue_length: u64,
    request_queue_length: u64,
    orderbook_length: u64,
    cluster_type: ClusterType,
    has_market: bool
) {
    if has_market {
        error!("Market already opened");
        return;
    }

    info!("Opening market");
    dex::openbook::open_market(
        &project_dir,
        &rpc_client,
        &keypair,
        &project_config,
        &quote_mint,
        event_queue_length,
        request_queue_length,
        orderbook_length,
        cluster_type);
}

pub async fn remove_liquidity(
    rpc_client: &RpcClient,
    keypair: &Keypair,
    project_dir: String,
    project_market: String,
    cluster_type: ClusterType,
    has_market: bool,
    has_liquidity: bool
) {
    if has_market == false {
        error!("Market not opened");
        return;
    }

    if has_liquidity == false {
        error!("Liquidity not added");
        return;
    }

    let market_config: MarketConfig = match MarketConfig::from_config_file(&project_market) {
        Ok(c) => c,
        Err(e) => {
            error!("Error reading market config file: {:?}", e);
            return;
        }
    };

    let pool_info = LiquidityPoolInfo::build_with_rpc(
        &rpc_client,
        &market_config.base_mint,
        &market_config.quote_mint,
        cluster_type
    ).await.unwrap();

    info!("Removing liquidity");
    raydium::remove_liquidity(&rpc_client,
                              &keypair,
                              &project_dir,
                              &pool_info,
                              None,
                              None,
                              cluster_type);
}

pub async fn add_liquidity(
    rpc_client: &RpcClient,
    keypair: &Keypair,
    project_dir: String,
    project_config: &ProjectConfig,
    project_market: String,
    project_liquidity: String,
    amount: f64,
    pool_open_sec_interval: u64,
    cluster_type: ClusterType,
    has_market: bool,
    has_liquidity: bool) {
    if has_market == false {
        error!("Market not opened");
        return;
    }

    if has_liquidity {
        error!("Liquidity already added");
        return;
    }

    // load market config
    let market_config: MarketConfig = match MarketConfig::from_config_file(&project_market) {
        Ok(c) => c,
        Err(e) => {
            error!("Error reading market config file: {:?}", e);
            return;
        }
    };

    let mut liquidity_config: LiquidityConfig = LiquidityConfig {
        file_location: project_liquidity,
        amm_id: "".to_string(),
        amm_authority: "".to_string(),
        amm_open_orders: "".to_string(),
        lp_mint: "".to_string(),
        coin_vault: "".to_string(),
        pc_vault: "".to_string(),
        target_orders: "".to_string(),
        amm_config_id: "".to_string(),
        base_token_account: "".to_string(),
    };

    info!("Adding liquidity");
    info!("Liquidity Amount: {:?}", amount);

    let now = std::time::SystemTime::now();
    let since_epoch = now.duration_since(UNIX_EPOCH).expect("Time went backwards");
    let seconds = since_epoch.as_secs() + pool_open_sec_interval;

    raydium::add_liquidity(&rpc_client,
                           &keypair,
                           &project_dir,
                           &project_config,
                           &market_config,
                           &mut liquidity_config,
                           amount,
                           seconds,
                           cluster_type);
}


pub async fn get_pool_information(
    rpc_client: &RpcClient,
    mint: &str,
    quote_mint: &str,
    cluster_type: ClusterType
) {
    let mint_pub = Pubkey::from_str(&mint).unwrap();
    let quote_mint_pub = Pubkey::from_str(&quote_mint).unwrap();


    info!("Base Mint: {:?}", mint_pub);
    info!("Quote Mint: {:?}", quote_mint_pub);

    let market_state = match MarketStateLayoutV3::get_with_reqwest(&rpc_client.url(),
                                                                   &mint_pub,
                                                                   &quote_mint_pub,
                                                                   cluster_type
    ).await {
        Ok(a) => a,
        Err(e) => {
            error!("Error getting market state: {:?}", e);
            return;
        }
    };

    info!("market_state info: {:?}", market_state);

    let liquidity_state = match LiquidityStateLayoutV4::get_with_reqwest(&rpc_client.url(),
                                                                         &mint_pub,
                                                                         &quote_mint_pub,
                                                                         cluster_type)
        .await {
        Ok(a) => a,
        Err(e) => {
            error!("Error getting liquidity state: {:?}", e);
            return;
        }
    };

    info!("liquidity_state info: {:?}", liquidity_state);

    let pool_info = match LiquidityPoolInfo::build(liquidity_state, market_state, cluster_type) {
        Ok(a) => a,
        Err(e) => {
            error!("Error building pool info: {:?}", e);
            return;
        }
    };

    info!("pool_info: {:?}", pool_info);
}

pub async fn create_token(
    rpc_client: &RpcClient,
    keypair: &Keypair,
    project_dir: &str,
    config: &Config,
    project_config: &mut ProjectConfig,
    project_config_file: String,
    project_image: String,
    project_metadata: String,
    has_project_config: bool,
    has_metadata: bool,
    generate_wallet: bool,
    count: i32,
    airdrop: bool,
    percentage: f64,
    freeze: bool
) {
    if has_project_config == false {
        error!("Project config not found");
        return;
    }

    if project_config.token_keypair.is_empty() == false {
        error!("Token already created");
        return;
    }

    let account_pub = &keypair.pubkey();
    info!("Account: {}", account_pub.to_string());

    let balance = rpc_client.get_balance(account_pub).unwrap();
    info!("Wallet Balance: {:?} SOL", lamports_to_sol(balance));

    if lamports_to_sol(balance) < 0.021f64 {
        error!("Insufficient balance to create token. Requires at least 0.021 SOL");
        return;
    }

    let mut wallets: Vec<Keypair> = vec![];
    if project_config.wallets.is_empty() {
        if generate_wallet {
            for _ in 0..count {
                wallets.push(Keypair::new().insecure_clone());
            }
        }
        project_config.wallets = wallets.iter().map(|w| w.to_base58_string()).collect();
    } else {
        for w in project_config.wallets.iter() {
            wallets.push(Keypair::from_base58_string(w).insecure_clone());
        }
    }

    if project_config.token_keypair.is_empty() {
        let token_keypair = Keypair::new();
        project_config.token_keypair = token_keypair.to_base58_string();
    }

    if has_metadata == false {
        info!("Uploading: {:?}", project_image);

        // upload the metadata and image
        let image_cid = match api::nft_storage::upload(
            &config.nft_storage_api_key,
            &project_image
        ).await {
            Ok(a) => {
                info!("Uploaded image: https://{}.ipfs.nftstorage.link", a);
                a
            }
            Err(e) => {
                error!("Error uploading image: {:?}", e);
                return;
            }
        };

        match api::nft_storage::generate_metadata(
            &project_dir,
            &project_config.name,
            &project_config.symbol,
            &project_config.description,
            &format!("https://{}.ipfs.nftstorage.link", image_cid),
            &project_config.tags,
            &project_config.telegram
        ) {
            Ok(_) => {}
            Err(e) => {
                panic!("Error generating metadata: {:?}", e);
            }
        };

        let metadata_cid = match api::nft_storage::upload(
            &config.nft_storage_api_key,
            &project_metadata
        ).await {
            Ok(a) => {
                info!("Uploaded metadata: https://{}.ipfs.nftstorage.link", a);
                a
            }
            Err(e) => {
                error!("Error uploading metadata: {:?}", e);
                return;
            }
        };

        project_config.metadata_uri = format!("https://{}.ipfs.nftstorage.link", metadata_cid);
    }

    spl::token::create(&rpc_client, &keypair, &project_config, freeze);
    match std::fs::write(&project_config_file, serde_yaml::to_string(&project_config).unwrap()) {
        Ok(_) => {
            info!("Project config updated");
        },
        Err(e) => {
            error!("Error updating project config: {:?}", e);
            return;
        }
    }

    if airdrop {
        info!("Airdropping to wallets");
        spl::token::airdrop(&rpc_client, &keypair, &project_dir, project_config, percentage, false);
    }
}

pub async fn withdraw(
    rpc_client: &RpcClient,
    payer: &Keypair,
    project_config: &ProjectConfig,
    destination: Option<String>
) {
    let mut destination_pub = payer.pubkey();
    if destination.is_some() {
        destination_pub = Pubkey::from_str(&destination.unwrap()).unwrap();
    }

    info!("Withdrawing to: {}", destination_pub.to_string().bold().green());

    spl::token::send(rpc_client, &destination_pub, project_config);
}

pub async fn airdrop(
    rpc_client: &RpcClient,
    keypair: &Keypair,
    project_dir: String,
    project_config: &mut ProjectConfig,
    percentage: f64,
    has_project_config: bool,
    confirm: bool
) {
    if has_project_config == false {
        error!("Project config not found");
        return;
    }

    info!("Airdropping to wallets");
    spl::token::airdrop(&rpc_client,
                        &keypair,
                        &project_dir,
                        project_config,
                        percentage,
                        confirm);
}

pub async fn burn(
    rpc_client: &RpcClient,
    payer: &Keypair,
    project_config: &ProjectConfig,
    liquidity_config_location: &str,
    mint: &str,
    percentage: f64,
    burn_airdrop: bool,
    burn_single: bool,
    pay: bool,
    liquidity: bool
) {
    let mut mint_pub;
    if mint.eq("So11111111111111111111111111111111111111112") {
        let token_keypair = Keypair::from_base58_string(&project_config.token_keypair);
        mint_pub = token_keypair.pubkey();
    } else {
        mint_pub = Pubkey::from_str(&mint).unwrap();
    }

    if liquidity {
        info!("Burning liquidity");
        let liquidity_config = match LiquidityConfig::from_config_file(liquidity_config_location) {
            Ok(k) => k,
            Err(e) => {
                error!("Liquidity config error: {:?}", e);
                return;
            }
        };
        mint_pub = Pubkey::from_str(&liquidity_config.lp_mint).unwrap()
    }

    info!("Burning token: {}", mint_pub.to_string().green().bold());
    info!("Burn Percentage: {:?}", percentage);

    if burn_airdrop {
        info!("Burning airdrop wallets");
        for wallet in project_config.wallets.iter() {
            let wallet = Keypair::from_base58_string(wallet);
            info!("Burning wallet: {}", wallet.pubkey().to_string());
            if pay {
                spl::token::burn(&rpc_client, &payer, &wallet, &mint_pub, percentage);
            } else {
                spl::token::burn(&rpc_client, &wallet, &wallet, &mint_pub, percentage);
            }
        }
    }

    if burn_single {
        info!("Burning main wallet");
        spl::token::burn(&rpc_client, &payer, &payer, &mint_pub, percentage);
    }
}

pub async fn project_sell(
    rpc_client: &RpcClient,
    project_config: &ProjectConfig,
    mint: &str,
    sell_all: bool,
    wallet_count: i32,
    percent: f64,
    interval: f64,
    cluster_type: ClusterType,
    has_market: bool,
    has_liquidity: bool,
    has_project_config: bool,
) {
    let mint_pub;
    if mint.eq("So11111111111111111111111111111111111111112") {
        if has_project_config {
            let token_keypair = Keypair::from_base58_string(&project_config.token_keypair);
            mint_pub = token_keypair.pubkey();
        } else {
            error!("Cannot sell native SOL mint");
            return;
        }
    } else {
        mint_pub = Pubkey::from_str(&mint).unwrap();
    }

    if has_market == false {
        error!("Market not opened");
        return;
    }

    if has_liquidity == false {
        error!("Liquidity not added");
        return;
    }

    info!("Selling token: {}", mint_pub.to_string());

    let amm_pool_info = match LiquidityPoolInfo::build_with_request(&rpc_client.url(),
                                                                    &mint_pub.to_string(),
                                                                    "So11111111111111111111111111111111111111112",
                                                                    cluster_type).await {
        Ok(a) => a,
        Err(e) => {
            error!("Error getting amm pool info: {:?}", e);
            return;
        }
    };
    info!("AMM Pool Info: {:?}", amm_pool_info);

    let mut wallet_information: Vec<WalletInformation> = vec![];
    {
        if sell_all {
            info!("Selling all wallets");
            for i in 0..project_config.wallets.len() {
                let wsol_wallet = Keypair::from_base58_string(&project_config.wsol_wallets[i]);
                wallet_information.push(
                    spl::token::get_wallet_token_information(
                        &rpc_client, &project_config.wallets[i],
                        &wsol_wallet.pubkey(), &mint_pub
                    )
                );
            }
        } else {
            info!("Selling specific wallets. Wallet count: {:?}", wallet_count);
            for i in 0..wallet_count as usize {
                if i >= project_config.wallets.len() {
                    break;
                }

                let wallet = &project_config.wallets[i];
                let wsol_wallet = Keypair::from_base58_string(&project_config.wsol_wallets[i]);
                wallet_information.push(
                    spl::token::get_wallet_token_information(
                        &rpc_client, &wallet, &wsol_wallet.pubkey(), &mint_pub
                    )
                );
            }
        }
    }

    for wallet in wallet_information.iter() {
        swap::sell(
            &rpc_client,
            &wallet,
            (wallet.balance as f64 * (percent / 100f64)) as u64,
            &amm_pool_info,
            cluster_type
        );

        sleep(std::time::Duration::from_secs_f64(interval)).await;
    }
}

pub async fn auto_sell(
    wss_client: &WebSocketClient,
    rpc_client: &RpcClient,
    project_config: &ProjectConfig,
    mint: &str,
    quote_mint: &str,
    interval: f64,
    overhead: f64,
    percent: f64,
    withdraw: bool,
    destination: &Pubkey,
    cluster_type: ClusterType
) {
    let token_keypair = Keypair::from_base58_string(&project_config.token_keypair);
    let mut base_mint_pub = Pubkey::from_str(&mint).unwrap();
    let quote_mint_pub = Pubkey::from_str(&quote_mint).unwrap();

    if mint.eq("So11111111111111111111111111111111111111112") {
        base_mint_pub = token_keypair.pubkey();
    }

    info!("Auto selling");
    info!("Base Mint: {}", mint.green().bold());
    info!("Quote Mint: {}", quote_mint.green().bold());
    info!("Interval: {}", interval.to_string().green().bold());
    info!("Percentage: {}", percent.to_string().green().bold());

    let pool_data_sync = Arc::new(
        Mutex::new(utils::websocket::PoolChunk {
            liquidity_state: None,
            market_state: None,
            liquidity_amount: None,
            task_done: false,
        }));

    let mut wallet_information: Vec<WalletInformation> = vec![];
    {
        if project_config.wsol_wallets.len() != project_config.wallets.len() {
            error!("No WSOL wallet available. Airdrop first");
            return;
        }

        for i in 0..project_config.wallets.len() {
            let wsol_wallet = Keypair::from_base58_string(&project_config.wsol_wallets[i]);
            wallet_information.push(
                spl::token::get_wallet_token_information(
                    &rpc_client, &project_config.wallets[i],
                    &wsol_wallet.pubkey(),
                    &base_mint_pub
                )
            );
        }
    }
    debug!("Wallet Information: {:?}", wallet_information);

    let task_config = utils::websocket::TaskConfig {
        sell_percent: percent,
        sell_interval: interval,
        rpc_url: rpc_client.url(),
        buy_amount: 0.0,
        overhead,
    };

    WebSocketClient::wait_for_pool(pool_data_sync.clone(),
                                   &wss_client,
                                   &base_mint_pub,
                                   &quote_mint_pub,
                                   ClusterType::Devnet);

    WebSocketClient::run_task(|wallets: Vec<WalletInformation>,
                               task_config: &utils::websocket::TaskConfig,
                               liquidity_pool_info: &LiquidityPoolInfo,
                               cluster_type: ClusterType| {
        debug!("run_task: {:?}", liquidity_pool_info);
        info!("Auto Selling");

        let connection = RpcClient::new(&task_config.rpc_url);

        // have n-seconds break from selling because you will drain it so fast
        if task_config.overhead > 0f64 {
            std::thread::sleep(std::time::Duration::from_secs_f64(task_config.overhead));
        }

        for wallet in wallets.iter() {
            info!("Selling wallet: {:?}", wallet.wallet.green().bold());
            swap::sell(
                &connection,
                &wallet,
                (wallet.balance as f64 * (task_config.sell_percent / 100f64)) as u64,
                &liquidity_pool_info,
                cluster_type
            );
            std::thread::sleep(std::time::Duration::from_secs_f64(task_config.sell_interval));
        }

        if withdraw {
            info!("Withdrawing");
            std::thread::sleep(std::time::Duration::from_secs(5));
            check_balance(&connection, None, &project_config, false);

            spl::token::send(
                &connection,
                &destination,
                &project_config
            );
        }
    }, wallet_information, task_config, cluster_type, pool_data_sync.clone()).await;
}

pub async fn sell(rpc_client: &RpcClient,
                  payer: &Keypair,
                  base_mint: &String,
                  quote_mint: &String,
                  percent: f64,
                  skip: bool,
                  cluster_type: ClusterType) {
    if base_mint.eq("So11111111111111111111111111111111111111112") {
        error!("Cannot sell native SOL mint");
        return;
    }

    let base_mint_pub = Pubkey::from_str(base_mint).unwrap();
    let quote_mint_pub = Pubkey::from_str(quote_mint).unwrap();
    let amm_info = match LiquidityPoolInfo::build_with_rpc(
        &rpc_client,
        &base_mint_pub.to_string(),
        &quote_mint_pub.to_string(),
        cluster_type
    ).await {
        Ok(a) => a,
        Err(e) => {
            error!("Error getting amm info: {:?}", e);
            return;
        }
    };

    info!("Selling token: {}", base_mint.green().bold());
    info!("Percent: {}", percent.to_string().green().bold());

    let (mut wsol_token_account, wsol_account_instruction) = spl::get_token_account(
        &rpc_client,
        &payer.pubkey(),
        &payer.pubkey(),
        &quote_mint_pub
    );

    if skip == false {
        if wsol_account_instruction.is_some() {
            (wsol_token_account, _) = spl::token::create_wsol_account(
                &rpc_client,
                &payer,
                0.001, // gas fee
                false
            );
        }
    }

    let wallet_information = spl::token::get_wallet_token_information(
        &rpc_client,
        &payer.to_base58_string(),
        &wsol_token_account,
        &base_mint_pub,
    );

    if wallet_information.balance == 0 {
        error!("Wallet has no token balance");
        return;
    }

    swap::sell(
        &rpc_client,
        &wallet_information,
        (wallet_information.balance as f64 * (percent / 100f64)) as u64,
        &amm_info,
        cluster_type
    );
}

pub async fn buy(rpc_client: &RpcClient,
                 wss_client: &WebSocketClient,
                 payer: &Keypair,
                 base_mint: &String,
                 quote_mint: &String,
                 amount: f64,
                 wait: bool,
                 skip: bool,
                 overhead: f64,
                 cluster_type: ClusterType) {
    if base_mint.eq("So11111111111111111111111111111111111111112") {
        error!("Cannot buy native SOL mint");
        return;
    }

    let base_mint_pub = Pubkey::from_str(base_mint).unwrap();
    let quote_mint_pub = Pubkey::from_str(quote_mint).unwrap();

    info!("Buying token: {}", base_mint.green().bold());
    info!("Amount: {}", amount.to_string().green().bold());

    let balance = rpc_client.get_balance(&payer.pubkey()).unwrap();
    info!("Wallet Balance: {:?} SOL", lamports_to_sol(balance));

    let mut wsol_token_account;
    let create_token_account_instruction;
    if skip == false {
        (wsol_token_account, create_token_account_instruction) = spl::get_token_account(
            &rpc_client,
            &payer.pubkey(),
            &payer.pubkey(),
            &spl_token::native_mint::id()
        );

        let mut enough_balance = true;
        if lamports_to_sol(balance) < (amount + 0.00011) {
            enough_balance = false;
        }

        let mut balance: u64 = 0u64;
        let b = rpc_client.get_token_account_balance(&wsol_token_account);
        if b.is_ok() {
            let b = b.unwrap();
            let decimal = b.decimals;
            balance = (b.ui_amount.unwrap() * 10f64.powf(decimal as f64)) as u64;
        }

        if balance >= sol_to_lamports(amount + 0.00011) {
            info!("WSOL Account has sufficient balance to buy token");
        } else {
            if enough_balance == false {
                error!("Insufficient balance to buy token. Requires at least {} SOL",
                    (amount + 0.00011).to_string().red());
                return;
            }

            if create_token_account_instruction.is_some() {
                info!("Creating WSOL account");
            } else {
                info!("Closing WSOL account and creating a new one");
                spl::token::close_wsol_account(
                    &rpc_client,
                    &payer,
                    &wsol_token_account.clone());
            }

            (wsol_token_account, _) = create_wsol_account(
                &rpc_client,
                &payer,
                amount + 0.00011,
                true
            );
        }
    } else {
        info!("Skipping WSOL account creation");
        (wsol_token_account, _) = spl::get_token_account(
            &rpc_client,
            &payer.pubkey(),
            &payer.pubkey(),
            &spl_token::native_mint::id()
        );
    }

    let (token_account, create_token_account_instruction) = spl::get_token_account(
        &rpc_client,
        &payer.pubkey(),
        &payer.pubkey(),
        &base_mint_pub
    );

    info!("Token Account: {}", token_account.to_string().green().bold());
    info!("WSOL Token Account: {}", wsol_token_account.to_string().green().bold());
    info!("Will Create New Token ATA: {}", create_token_account_instruction.is_some());

    let wallet_information = WalletInformation {
        wallet: payer.to_base58_string(),
        wsol_account: wsol_token_account,
        token_account,
        balance: 0,
        create_token_account_instruction,
    };

    let amm_info = LiquidityPoolInfo::build_with_rpc(
        &rpc_client,
        &base_mint_pub.to_string(),
        &quote_mint_pub.to_string(),
        cluster_type
    ).await;

    if wait && amm_info.is_err() {
        let pool_data_sync = Arc::new(
            Mutex::new(utils::websocket::PoolChunk {
                liquidity_state: None,
                market_state: None,
                liquidity_amount: None,
                task_done: false,
            }));

        let task_config = utils::websocket::TaskConfig {
            sell_percent: 0f64,
            sell_interval: 0f64,
            rpc_url: rpc_client.url(),
            buy_amount: amount,
            overhead,
        };

        WebSocketClient::wait_for_pool(pool_data_sync.clone(),
                                       &wss_client,
                                       &base_mint_pub,
                                       &quote_mint_pub,
                                       cluster_type);

        WebSocketClient::run_task(|wallets: Vec<WalletInformation>,
                                   task_config: &utils::websocket::TaskConfig,
                                   liquidity_pool_info: &LiquidityPoolInfo,
                                   cluster_type: ClusterType| {
            debug!("run_task: {:?}", liquidity_pool_info);
            info!("Buying");

            let connection = RpcClient::new(&task_config.rpc_url);

            // have n-seconds break from selling because you will drain it so fast
            // if task_config.overhead > 0f64 {
            //     std::thread::sleep(std::time::Duration::from_secs_f64(task_config.overhead));
            // }

            for wallet in wallets.iter() {
                swap::buy(
                    &connection,
                    &wallet,
                    task_config.buy_amount,
                    liquidity_pool_info,
                    cluster_type
                );
            }
        }, vec![wallet_information.clone()],
                                  task_config,
                                  cluster_type,
                                  pool_data_sync.clone()).await;
    } else {
        debug!("AMM Info: {:?}", amm_info);
        swap::buy(
            &rpc_client,
            &wallet_information,
            amount,
            &amm_info.unwrap(),
            cluster_type
        );
    }
}

pub async fn generate_wallets(project_config_file: &str,
                              project_config: &mut ProjectConfig,
                              count: i32,
                              replace: bool) {
    let mut wallets: Vec<Keypair> = vec![];
    for _ in 0..count {
        wallets.push(Keypair::new().insecure_clone());
    }

    let wallet_bs58 = wallets.iter().map(|w| w.to_base58_string()).collect();
    if replace {
        project_config.wallets = wallet_bs58;
    } else {
        project_config.wallets.extend(wallet_bs58);
    }

    match std::fs::write(&project_config_file, serde_yaml::to_string(&project_config).unwrap()) {
        Ok(_) => {
            info!("Project config updated");
        },
        Err(e) => {
            error!("Error updating project config: {:?}", e);
            return;
        }
    }
}

pub fn check_balance(rpc_client: &RpcClient, payer: Option<&Keypair>, project_config: &ProjectConfig, all: bool) {
    let mut total_balance = 0u64;

    if all {
        if payer.is_some() {
            let key = payer.unwrap();
            let balance = rpc_client.get_balance(&key.pubkey()).unwrap();
            total_balance += balance;
            info!("Wallet: {:?} Balance: {:?} SOL",
                key.pubkey().to_string().green().bold(),
                lamports_to_sol(balance).to_string().green().bold());
        }
        total_balance += check_wsol_balance(rpc_client, project_config);
    }

    for wallet in project_config.wallets.iter() {
        let key = Keypair::from_base58_string(&wallet);
        let balance = rpc_client.get_balance(&key.pubkey()).unwrap();
        total_balance += balance;
        info!("Wallet: {} Balance: {} SOL", key.pubkey().to_string().green().bold(),
            lamports_to_sol(balance).to_string().green().bold());
    }

    info!("Total Balance: {:?} SOL",
        lamports_to_sol(total_balance).to_string().green().bold());
}

pub fn check_wsol_balance(rpc_client: &RpcClient, project_config: &ProjectConfig) -> u64 {
    let mut total_balance = 0u64;
    for wallet in project_config.wsol_wallets.iter() {
        let key = Keypair::from_base58_string(&wallet);

        let mut balance: u64 = 0u64;
        let mut tries: u64 = 0u64;
        loop {
            if tries >= 5 {
                break;
            }

            let b = match rpc_client.get_token_account_balance(&key.pubkey()) {
                Ok(a) => a,
                Err(_) => {
                    break;
                }
            };

            let decimal = b.decimals;
            balance = (b.ui_amount.unwrap() * 10f64.powf(decimal as f64)) as u64;
            if balance > 1 {
                break;
            }
            tries += 1;
        }

        total_balance += balance;
        info!("WSOL Wallet: {:?} Balance: {:?} SOL",
            key.pubkey().to_string().green().bold(),
            lamports_to_sol(balance).to_string().green().bold());
    }

    info!("Total Balance: {:?} SOL", lamports_to_sol(total_balance).to_string().green().bold());

    total_balance
}

pub async fn revoke_mint_authority(rpc_client: &RpcClient, payer: &Keypair, project_config: &ProjectConfig) {
    info!("Revoking mint authority of {}", project_config.name);
    spl::token::revoke_mint_authority(&rpc_client, &payer, &project_config);
}

pub async fn create_wsol(rpc_client: &RpcClient, project_dir: &str,
                         project_config: &mut ProjectConfig,
                         amount: f64, skip_confirmation: bool) {
    if project_config.wsol_wallets.len() == 0 {
        for wallet in project_config.wallets.iter() {
            let wallet = Keypair::from_base58_string(wallet);

            let (_, wsol_keypair) = create_wsol_account(
                &rpc_client,
                &wallet,
                amount,
                !skip_confirmation
            );
            project_config.wsol_wallets.push(wsol_keypair.to_base58_string());
        }

        match std::fs::write(
            format!("{}/config.yaml", project_dir),
            serde_yaml::to_string(&project_config).unwrap()
        ) {
            Ok(_) => {
                info!("Project config updated");
            }
            Err(e) => {
                error!("Error writing project config, {:?}", e);
            }
        }
    }
}

pub async fn monitor_account(wss_client: &WebSocketClient, address: &str) {
    info!("Monitoring account changes for {}", address.bold().green());
    wss_client.wss_monitor_liquidity_changes(address);
}

pub async fn rug_token(wss_pool_client: &WebSocketClient,
                       wss_liquidity_client: &WebSocketClient,
                       token_creator: &Keypair,
                       rpc_client: &RpcClient,
                       project_config: &ProjectConfig,
                       initial_liquidity: f64,
                       target_liquidity: f64,
                       cluster_type: ClusterType) {
    let token_keypair = Keypair::from_base58_string(&project_config.token_keypair);
    let base_mint_pub = token_keypair.pubkey();
    let quote_mint_pub = spl_token::native_mint::id();

    info!("Auto Rugging");
    info!("Base Mint: {}", base_mint_pub.to_string().green().bold());
    info!("Initial Liquidity: {} SOL", initial_liquidity.to_string().green().bold());
    info!("Target Liquidity: {} SOL", target_liquidity.to_string().green().bold());
    info!("Increase: {} SOL", (target_liquidity - initial_liquidity).to_string().green().bold());

    let pool_data_sync = Arc::new(
        Mutex::new(utils::websocket::PoolChunk {
            liquidity_state: None,
            market_state: None,
            liquidity_amount: None,
            task_done: false,
        }));

    let task_config = LiquidityTaskConfig {
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
        &wss_pool_client,
        &wss_liquidity_client,
        &base_mint_pub,
        &quote_mint_pub,
        cluster_type,
        market_state,
        liquidity_state
    );


    WebSocketClient::run_liquidity_change_task(
        |rpc_client: &RpcClient, wallet_information: &WalletInformation, owner: &Keypair, liquidity_pool_info: &LiquidityPoolInfo, cluster_type: ClusterType| {
            raydium::remove_liquidity(rpc_client, owner, "",
                                      liquidity_pool_info,
                                      Some(wallet_information.token_account),
                                      Some(wallet_information.balance),
                                      cluster_type);
        },
        rpc_client,
        &token_creator,
        task_config.clone(),
        cluster_type,
        pool_data_sync.clone()
    ).await;
}