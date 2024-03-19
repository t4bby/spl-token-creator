use std::str::FromStr;
use colored::Colorize;
use log::{debug, error, info};
use solana_client::rpc_client::RpcClient;
use solana_client::rpc_config::RpcSendTransactionConfig;
use solana_program::instruction::{AccountMeta, Instruction};
use solana_program::program_pack::Pack;
use solana_program::pubkey::Pubkey;
use solana_sdk::commitment_config::{CommitmentConfig, CommitmentLevel};
use solana_sdk::genesis_config::ClusterType;
use solana_sdk::signature::{Keypair, Signer};
use crate::dex::raydium::{AMM_PROGRAM_DEV_ID, AMM_PROGRAM_ID};
use crate::dex::raydium::layout::SwapLayout;
use crate::dex::raydium::pool::LiquidityPoolInfo;
use crate::spl;
use crate::spl::token::{WalletInformation};

pub fn make_swap_instruction(
    amount: u64,
    token_program_id: &Pubkey,
    program_id: &Pubkey,
    min_amount_out: u64,
    token_in: &Pubkey,
    token_out: &Pubkey,
    amm_info: &LiquidityPoolInfo,
    payer: &Pubkey,
) -> Instruction {

    // accounts is a list of accounts that the instruction will use
    let accounts = vec![
        AccountMeta::new_readonly(*token_program_id, false),
        AccountMeta::new(amm_info.id, false),
        AccountMeta::new_readonly(amm_info.authority, false),
        AccountMeta::new(amm_info.open_orders, false),
        AccountMeta::new(amm_info.target_orders, false),
        AccountMeta::new(amm_info.base_vault, false),
        AccountMeta::new(amm_info.quote_vault, false),
        AccountMeta::new_readonly(amm_info.market_program_id, false),
        AccountMeta::new(amm_info.market_id, false),
        AccountMeta::new(amm_info.bids, false),
        AccountMeta::new(amm_info.asks, false),
        AccountMeta::new(amm_info.event_queue, false),
        AccountMeta::new(amm_info.market_base_vault, false),
        AccountMeta::new(amm_info.market_quote_vault, false),
        AccountMeta::new_readonly(amm_info.market_authority, false),
        AccountMeta::new(*token_in, false),
        AccountMeta::new(*token_out, false),
        AccountMeta::new_readonly(*payer, true),
    ];

    // layout is a SwapLayout struct that contains the instruction data
    let layout = SwapLayout {
        instruction: 9,
        amount_in: amount,
        min_amount_out,
    };

    debug!("Swap Layout: {:?}", layout);

    let mut data = vec![0u8; SwapLayout::LEN];
    layout.pack_into_slice(&mut data);

    // return an Instruction struct that contains the program_id, accounts, and data
    Instruction::new_with_bytes(*program_id, &data, accounts)
}

pub fn swap_instruction(
    program_id: &Pubkey,
    payer: &Keypair,
    in_token: &Pubkey,
    out_token: &Pubkey,
    amount: f64,
    decimal: u8,
    amm_info: &LiquidityPoolInfo,
) -> Vec<Instruction> {
    let mut instructions = vec![];

    instructions.push(
        solana_sdk::compute_budget::ComputeBudgetInstruction::set_compute_unit_price(44684)
    );

    instructions.push(
        solana_sdk::compute_budget::ComputeBudgetInstruction::set_compute_unit_limit(600000)
    );

    instructions.push(
        make_swap_instruction(
            (amount * 10f64.powi(decimal as i32)) as u64,
            &spl_token::id(),
            &program_id,
            0,
            &in_token,
            &out_token,
            amm_info,
            &payer.pubkey(),
        )
    );

    instructions
}

#[allow(deprecated, unused)]
pub fn get_or_create_token_account(
    rpc_client: &RpcClient,
    payer: &Keypair,
    mint_pubkey: &Pubkey
) -> Pubkey {
    info!("Get or Create Token Account: {:?}", mint_pubkey);
    let (token_account, token_account_instruction) =
        spl::get_token_account(rpc_client, &payer.pubkey(), &payer.pubkey(), mint_pubkey);

    if token_account_instruction.is_some() {
        info!("Creating Token Account");
        let transaction = solana_sdk::transaction::Transaction::new_signed_with_payer(
            &[token_account_instruction.unwrap()],
            Some(&payer.pubkey()),
            &[&payer],
            rpc_client.get_recent_blockhash().unwrap().0
        );

        match rpc_client.send_and_confirm_transaction_with_spinner_and_commitment(&transaction,
                                                                                  CommitmentConfig::confirmed()) {
            Ok(s) => {
                info!("Create Token Account Tx: {}", s.to_string().bold().green());
            }
            Err(e) => {
                panic!("Error creating token account: {:?}", e)
            }
        }
    }
    info!("Token Account: {}", token_account.to_string());

    token_account
}

#[allow(deprecated)]
pub fn buy(
    rpc_client: &RpcClient,
    payer: &WalletInformation,
    amount: f64,
    amm_info: &LiquidityPoolInfo,
    cluster_type: ClusterType
) {
    let program_id: Pubkey;
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

    debug!("Wallet information: {:?}", payer);
    let wallet = Keypair::from_base58_string(&payer.wallet);

    info!("Creating instructions");
    let mut instructions: Vec<Instruction> = vec![];

    if payer.create_token_account_instruction.is_some() {
        info!("Adding token account creation instruction");
        instructions.push(
            payer.clone().create_token_account_instruction.unwrap()
        )
    }

    instructions.extend(
        swap_instruction(
            &program_id,
            &wallet,
            &payer.wsol_account,
            &payer.token_account,
            amount,
            9, // WSOL decimal
            amm_info
        )
    );

    let transaction = solana_sdk::transaction::Transaction::new_signed_with_payer(
        &instructions,
        Some(&wallet.pubkey()),
        &[&wallet],
        rpc_client.get_recent_blockhash().unwrap().0
    );

    info!("Sending transaction");
    match rpc_client.send_and_confirm_transaction_with_spinner_and_commitment(&transaction,
                                                                              CommitmentConfig::processed()) {
        Ok(s) => {
            info!("Buy Tx: {}", s.to_string().bold().green());
        }
        Err(e) => {
            error!("{:?}", e);
        }
    }
}

#[allow(deprecated)]
pub fn sell(rpc_client: &RpcClient,
            payer: &WalletInformation,
            amount: u64,
            amm_info: &LiquidityPoolInfo,
            cluster_type: ClusterType) {
    let wallet = Keypair::from_base58_string(&payer.wallet);
    let mut instructions: Vec<Instruction> = vec![];
    info!("Selling on wallet: {}", wallet.pubkey().to_string());

    let program_id: Pubkey;
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

    instructions.push(
        solana_sdk::compute_budget::ComputeBudgetInstruction::set_compute_unit_price(44684)
    );

    instructions.push(
        solana_sdk::compute_budget::ComputeBudgetInstruction::set_compute_unit_limit(600000)
    );

    instructions.push(
        make_swap_instruction(
            amount,
            &spl_token::id(),
            &program_id,
            0,
            &payer.token_account,
            &payer.wsol_account,
            amm_info,
            &wallet.pubkey(),
        )
    );

    instructions.push(
        spl_token::instruction::close_account(
            &spl_token::id(),
            &payer.token_account,
            &wallet.pubkey(),
            &wallet.pubkey(),
            &[]
        ).unwrap()
    );

    let transaction = solana_sdk::transaction::Transaction::new_signed_with_payer(
        &instructions,
        Some(&wallet.pubkey()),
        &[&wallet],
        rpc_client.get_recent_blockhash().unwrap().0
    );

    match rpc_client.send_transaction_with_config(&transaction, RpcSendTransactionConfig {
        skip_preflight: true,
        preflight_commitment: Some(CommitmentLevel::Confirmed),
        encoding: None,
        max_retries: None,
        min_context_slot: None,
    }) {
        Ok(s) => {
            info!("Sell Tx: {}", s.to_string().bold().green());
        }
        Err(e) => {
            error!("{:?}", e);
        }
    }
}