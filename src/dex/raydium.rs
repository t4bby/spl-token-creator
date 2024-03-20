pub mod layout;
pub mod error;
pub mod pool;
pub mod swap;

use std::str::FromStr;
use std::time::UNIX_EPOCH;
use colored::Colorize;
use log::{error, info};
use solana_client::rpc_client::RpcClient;
use solana_client::rpc_config::RpcSendTransactionConfig;
use solana_program::instruction::{AccountMeta, Instruction};
use solana_program::native_token::LAMPORTS_PER_SOL;
use solana_program::pubkey::Pubkey;
use solana_sdk::commitment_config::CommitmentLevel;
use solana_sdk::genesis_config::ClusterType;
use solana_sdk::signature::{Keypair, Signer};
use crate::cli::config::{LiquidityConfig, MarketConfig, ProjectConfig};
use crate::{dex, spl};
use crate::dex::raydium::layout::{RaydiumInstruction, RemoveLiquidityLayout};
use crate::dex::raydium::pool::LiquidityPoolInfo;

pub const AMM_PROGRAM_ID: &str = "675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8";
pub const AMM_PROGRAM_DEV_ID: &str = "HWy1jotHpo6UqeQxx49dpYYdQB8wj9Qk9MdxwjLvDHB8";
pub const AUTORITY_ID: &str = "5Q544fKrFoe6tsEbD7S8EmxGTJYAKtTVhAW5Q5pge4j1";

pub fn make_remove_liquidity_instruction(
    program_id: &Pubkey,
    lp_token_account: &Pubkey,
    base_token_account: &Pubkey,
    quote_token_account: &Pubkey,
    payer: &Pubkey,
    amount: u64,
    amm_id: &Pubkey,
    amm_authority: &Pubkey,
    amm_open_orders: &Pubkey,
    amm_target_orders: &Pubkey,
    lp_mint: &Pubkey,
    base_vault: &Pubkey,
    quote_vault: &Pubkey,
    withdraw_queue: &Pubkey,
    lp_vault: &Pubkey,
    market_program_id: &Pubkey,
    market_id: &Pubkey,
    market_base_vault: &Pubkey,
    market_quote_vault: &Pubkey,
    market_authority: &Pubkey,
    market_event_queue: &Pubkey,
    market_bids: &Pubkey,
    market_asks: &Pubkey,
) -> Instruction {
    let data = RemoveLiquidityLayout {
        instruction: 4,
        amount_in: amount,
    };

    let meta_data = vec![
        AccountMeta::new_readonly(spl_token::id(), false),
        AccountMeta::new(*amm_id, false),
        AccountMeta::new_readonly(*amm_authority, false),
        AccountMeta::new(*amm_open_orders, false),
        AccountMeta::new(*amm_target_orders, false),
        AccountMeta::new(*lp_mint, false),
        AccountMeta::new(*base_vault, false),
        AccountMeta::new(*quote_vault, false),

        // V4
        AccountMeta::new(*withdraw_queue, false),
        AccountMeta::new(*lp_vault, false),

        // serum
        AccountMeta::new_readonly(*market_program_id, false),
        AccountMeta::new(*market_id, false),
        AccountMeta::new(*market_base_vault, false),
        AccountMeta::new(*market_quote_vault, false),
        AccountMeta::new_readonly(*market_authority, false),

        // user
        AccountMeta::new(*lp_token_account, false),
        AccountMeta::new(*base_token_account, false),
        AccountMeta::new(*quote_token_account, false),
        AccountMeta::new(*payer, true),

        // serum orderbook
        AccountMeta::new(*market_event_queue, false),
        AccountMeta::new(*market_bids, false),
        AccountMeta::new(*market_asks, false),
    ];

    Instruction::new_with_borsh(*program_id, &data, meta_data)
}

pub fn make_create_pool_v4_instruction(program_id: &Pubkey,
                                       amm_id: &Pubkey,
                                       amm_authority: &Pubkey,
                                       amm_open_orders: &Pubkey,
                                       lp_mint: &Pubkey,
                                       coin_mint: &Pubkey,
                                       pc_mint: &Pubkey,
                                       coin_vault: &Pubkey,
                                       pc_vault: &Pubkey,
                                       amm_target_orders: &Pubkey,
                                       market_program_id: &Pubkey,
                                       market_id: &Pubkey,
                                       user_wallet: &Pubkey,
                                       user_coin_vault: &Pubkey,
                                       user_pc_vault: &Pubkey,
                                       user_lp_vault: &Pubkey,
                                       nonce: u8,
                                       open_time: u64,
                                       coin_amount: u64,
                                       pc_amount: u64,
                                       amm_config_id: &Pubkey,
                                       fee_destination_id: &Pubkey)
                                       -> Instruction {
    let meta_data = vec![
        AccountMeta::new_readonly(spl_token::id(), false),
        AccountMeta::new_readonly(spl_associated_token_account::id(), false),
        AccountMeta::new_readonly(Pubkey::from_str("11111111111111111111111111111111").unwrap(), false),
        AccountMeta::new_readonly(solana_program::sysvar::rent::id(), false),
        AccountMeta::new(*amm_id, false),
        AccountMeta::new_readonly(*amm_authority, false),
        AccountMeta::new(*amm_open_orders, false),
        AccountMeta::new(*lp_mint, false),
        AccountMeta::new(*coin_mint, false),
        AccountMeta::new_readonly(*pc_mint, false),
        AccountMeta::new(*coin_vault, false),
        AccountMeta::new(*pc_vault, false),
        AccountMeta::new(*amm_target_orders, false),
        AccountMeta::new_readonly(*amm_config_id, false),
        AccountMeta::new(*fee_destination_id, false),
        AccountMeta::new_readonly(*market_program_id, false),
        AccountMeta::new_readonly(*market_id, false),
        AccountMeta::new(*user_wallet, true),
        AccountMeta::new(*user_coin_vault, false),
        AccountMeta::new(*user_pc_vault, false),
        AccountMeta::new(*user_lp_vault, false),
    ];

    let data = RaydiumInstruction {
        instruction: 1,
        nonce,
        open_time,
        pc_amount,
        coin_amount,
    };

    Instruction::new_with_borsh(*program_id, &data, meta_data)
}

#[allow(deprecated)]
pub async fn remove_liquidity(rpc_client: &RpcClient,
                              payer: &Keypair,
                              project_dir: &str,
                              liquidity_pool_info: &LiquidityPoolInfo,
                              cluster_type: ClusterType) {
    let program_id;

    match cluster_type {
        ClusterType::MainnetBeta => {
            program_id = Pubkey::from_str(AMM_PROGRAM_ID).unwrap();
        }
        ClusterType::Devnet => {
            program_id = Pubkey::from_str(AMM_PROGRAM_DEV_ID).unwrap();
        }
        _ => {
            unimplemented!()
        }
    }


    let mut instructions: Vec<Instruction> = vec![];

    instructions.push(
        solana_sdk::compute_budget::ComputeBudgetInstruction::set_compute_unit_price(773552)
    );

    instructions.push(
        solana_sdk::compute_budget::ComputeBudgetInstruction::set_compute_unit_limit(500000)
    );

    let balance_needed = rpc_client.get_minimum_balance_for_rent_exemption(165).unwrap();

    let (new_token_account, seed) = spl::generate_pubkey(&payer.pubkey(), &spl_token::id(), &project_dir);
    info!("Seed: {:?}", seed);
    info!("New Token Account: {:?}", new_token_account);

    instructions.push(
        spl::create_account_with_seed(
            &payer.pubkey(),
            &new_token_account,
            &payer.pubkey(),
            &seed,
            balance_needed,
            165,
            &spl_token::id(),
        )
    );

    instructions.push(
        spl::create_initialize_account_instruction(
            &spl_token::id(),
            &spl_token::native_mint::id(),
            &new_token_account,
            &payer.pubkey(),
        )
    );

    let (user_coin_token_account, _) = spl::get_token_account(
        rpc_client,
        &payer.pubkey(),
        &payer.pubkey(),
        &liquidity_pool_info.liquidity_state.base_mint
    );


    let (lp_token_account, _) = spl::get_token_account(rpc_client,
                                                       &payer.pubkey(),
                                                       &payer.pubkey(),
                                                       &liquidity_pool_info.lp_mint);

    let mut balance: u64 = 0u64;
    let b = rpc_client.get_token_account_balance(&lp_token_account);
    if b.is_ok() {
        let b = b.unwrap();
        let decimal = b.decimals;
        balance = (b.ui_amount.unwrap() * 10f64.powf(decimal as f64)) as u64;
    }

    instructions.push(
        make_remove_liquidity_instruction(
            &program_id,
            &lp_token_account,
            &user_coin_token_account,
            &new_token_account,
            &payer.pubkey(),
            balance,
            &liquidity_pool_info.id,
            &liquidity_pool_info.authority,
            &liquidity_pool_info.open_orders,
            &liquidity_pool_info.target_orders,
            &liquidity_pool_info.lp_mint,
            &liquidity_pool_info.base_vault,
            &liquidity_pool_info.quote_vault,
            &liquidity_pool_info.liquidity_state.withdraw_queue,
            &liquidity_pool_info.liquidity_state.lp_vault,
            &liquidity_pool_info.market_program_id,
            &liquidity_pool_info.market_id,
            &liquidity_pool_info.market_base_vault,
            &liquidity_pool_info.market_quote_vault,
            &liquidity_pool_info.market_authority,
            &liquidity_pool_info.event_queue,
            &liquidity_pool_info.bids,
            &liquidity_pool_info.asks,
        )
    );

    instructions.push(
        spl_token::instruction::close_account(
            &spl_token::id(),
            &new_token_account,
            &payer.pubkey(),
            &payer.pubkey(),
            &[]
        ).unwrap()
    );

    let transaction = solana_sdk::transaction::Transaction::new_signed_with_payer(
        &instructions,
        Some(&payer.pubkey()),
        &[&payer],
        rpc_client.get_recent_blockhash().unwrap().0
    );

    match rpc_client.send_transaction_with_config(&transaction, RpcSendTransactionConfig {
        skip_preflight: false,
        preflight_commitment: Some(CommitmentLevel::Finalized),
        encoding: None,
        max_retries: None,
        min_context_slot: None,
    }) {
        Ok(s) => {
            info!("Liquidity Remove Tx: {}", s.to_string().bold().green());
        }
        Err(e) => {
            error!("Error: {:?}", e);
        }
    }
}

#[allow(deprecated)]
pub async fn add_liquidity(rpc_client: &RpcClient,
                           payer: &Keypair,
                           project_dir: &str,
                           project_config: &ProjectConfig,
                           market_config: &MarketConfig,
                           liquidity_config: &mut LiquidityConfig,
                           amount: f64,
                           cluster_type: ClusterType) {
    let amount = (LAMPORTS_PER_SOL as f64 * amount) as u64;
    let token_keypair = Keypair::from_base58_string(&project_config.token_keypair);
    let market_keypair = Keypair::from_base58_string(&market_config.market_keypair);

    let program_id;
    let market_program_id;
    let create_pool_fee_address ;

    match cluster_type {
        ClusterType::MainnetBeta => {
            program_id = Pubkey::from_str(AMM_PROGRAM_ID).unwrap();
            market_program_id = Pubkey::from_str(dex::openbook::SERUM_PROGRAM_ID).unwrap();
            create_pool_fee_address = Pubkey::from_str("7YttLkHDoNj9wyDur5pM1ejNaAvT9X4eqaYcHQqtj2G5").unwrap();
        }
        ClusterType::Devnet => {
            program_id = Pubkey::from_str(AMM_PROGRAM_DEV_ID).unwrap();
            market_program_id = Pubkey::from_str(dex::openbook::SERUM_PROGRAM_DEV_ID).unwrap();
            create_pool_fee_address = Pubkey::from_str("3XMrhbv989VxAMi3DErLV9eJht1pHppW5LbKxe9fkEFR").unwrap();
        }
        _ => {
            unimplemented!()
        }
    }

    let wsol_pub = &spl_token::native_mint::id();

    let mut instructions: Vec<Instruction> = vec![];

    instructions.push(
        solana_sdk::compute_budget::ComputeBudgetInstruction::set_compute_unit_price(25000)
    );

    instructions.push(
        solana_sdk::compute_budget::ComputeBudgetInstruction::set_compute_unit_limit(600000)
    );

    let (amm_id, _) = LiquidityPoolInfo::get_associated_id(program_id,
                                                           market_keypair.pubkey());

    let (amm_authority, nonce) = LiquidityPoolInfo::get_associated_authority(program_id);

    let (amm_open_orders, _) = LiquidityPoolInfo::get_associated_open_orders(program_id,
                                                                             market_keypair.pubkey());

    let (lp_mint, _) = LiquidityPoolInfo::get_associated_lp_mint(program_id,
                                                                 market_keypair.pubkey());

    let (coin_vault, _) = LiquidityPoolInfo::get_associated_base_vault(program_id,
                                                                       market_keypair.pubkey());

    let (pc_vault, _) = LiquidityPoolInfo::get_associated_quote_vault(program_id,
                                                                      market_keypair.pubkey());

    let (target_orders, _) = LiquidityPoolInfo::get_associated_target_orders(program_id,
                                                                             market_keypair.pubkey());

    let (amm_config_id, _) = LiquidityPoolInfo::get_associated_config_id(program_id);

    let (base_token_account, _) = spl::get_token_account(&rpc_client,
                                                         &payer.pubkey(),
                                                         &payer.pubkey(),
                                                         &token_keypair.pubkey()
    );

    liquidity_config.amm_config_id = amm_config_id.to_string();
    liquidity_config.amm_id = amm_id.to_string();
    liquidity_config.amm_authority = amm_authority.to_string();
    liquidity_config.amm_open_orders = amm_open_orders.to_string();
    liquidity_config.lp_mint = lp_mint.to_string();
    liquidity_config.coin_vault = coin_vault.to_string();
    liquidity_config.pc_vault = pc_vault.to_string();
    liquidity_config.target_orders = target_orders.to_string();
    liquidity_config.base_token_account = base_token_account.to_string();

    match std::fs::write(&liquidity_config.file_location, serde_yaml::to_string(&liquidity_config).unwrap()) {
        Ok(_) => {
            info!("Liquidity config updated");
        }
        Err(e) => {
            panic!("Error: {:?}", e);
        }
    }

    let balance_needed = rpc_client.get_minimum_balance_for_rent_exemption(165).unwrap();

    let (new_token_account, seed) = spl::generate_pubkey(&payer.pubkey(), &spl_token::id(), &project_dir);
    info!("Seed: {:?}", seed);
    info!("New Token Account: {:?}", new_token_account);

    instructions.push(
        spl::create_account_with_seed(
            &payer.pubkey(),
            &new_token_account,
            &payer.pubkey(),
            &seed,
            amount + balance_needed,
            165,
            &spl_token::id(),
        )
    );

    instructions.push(
        spl::create_initialize_account_instruction(
            &spl_token::id(),
            &wsol_pub,
            &new_token_account,
            &payer.pubkey(),
        )
    );

    let mut balance: u64 = 0u64;
    let b = rpc_client.get_token_account_balance(&base_token_account);
    if b.is_ok() {
        let b = b.unwrap();
        let decimal = b.decimals;
        balance = (b.ui_amount.unwrap() * 10f64.powf(decimal as f64)) as u64;
    }

    let now = std::time::SystemTime::now();
    let since_epoch = now.duration_since(UNIX_EPOCH).expect("Time went backwards");
    let seconds = since_epoch.as_secs();

    instructions.push(
        make_create_pool_v4_instruction(
            &program_id,
            &amm_id,
            &amm_authority,
            &amm_open_orders,
            &lp_mint,
            &token_keypair.pubkey(),
            &wsol_pub,
            &coin_vault,
            &pc_vault,
            &target_orders,
            &market_program_id,
            &market_keypair.pubkey(),
            &payer.pubkey(),
            &base_token_account,
            &new_token_account,
            &spl::get_associated_token_address(
                &payer.pubkey(),
                &lp_mint
            ),
            nonce,
            seconds,
            balance,
            amount,
            &amm_config_id,
            &create_pool_fee_address
        )
    );

    instructions.push(
        spl_token::instruction::close_account(
            &spl_token::id(),
            &new_token_account,
            &payer.pubkey(),
            &payer.pubkey(),
            &[]
        ).unwrap()
    );

    let transaction = solana_sdk::transaction::Transaction::new_signed_with_payer(
        &instructions,
        Some(&payer.pubkey()),
        &[&payer],
        rpc_client.get_recent_blockhash().unwrap().0
    );

    match rpc_client.send_transaction_with_config(&transaction, RpcSendTransactionConfig {
        skip_preflight: false,
        preflight_commitment: Some(CommitmentLevel::Finalized),
        encoding: None,
        max_retries: None,
        min_context_slot: None,
    }) {
        Ok(s) => {
            info!("Add Liquidity Tx: {}", s.to_string().bold().green());
        }
        Err(e) => {
            error!("Error: {:?}", e);
        }
    }
}