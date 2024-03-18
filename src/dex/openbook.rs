use std::str::FromStr;
use bumpalo::{Bump};
use log::{error, info};
use solana_client::rpc_client::RpcClient;
use solana_client::rpc_config::RpcSendTransactionConfig;
use solana_program::instruction::Instruction;
use solana_program::message::{Message};
use solana_program::native_token::LAMPORTS_PER_SOL;
use solana_program::pubkey::Pubkey;
use solana_sdk::genesis_config::ClusterType;
use solana_sdk::signature::{Keypair, Signature, SignerError};
use solana_sdk::signer::Signer;
use solana_sdk::transaction::{Transaction};
use crate::cli::config::{MarketConfig, ProjectConfig};

pub const SERUM_PROGRAM_ID: &str = "srmqPvymJeFKQ4zGQed1GFppgkRHL9kaELCbyksJtPX";
pub const SERUM_PROGRAM_DEV_ID: &str = "EoTcMgcDRTJVZDMZWBoU6rhYHZfkNTVEAfz3uUJRcYGj";

pub const REQUEST_QUEUE_ITEM_SIZE: u64 = 80;
pub const EVENT_QUEUE_ITEM_SIZE: u64 = 88;
pub const ORDERBOOK_ITEM_SIZE: u64 = 72;
pub const QUEUE_HEADER_SIZE: u64 = 44;
pub const ORDERBOOK_HEADER_SIZE: u64 = 52;


fn calculate_request_queue_size(size: u64) -> u64 {
    REQUEST_QUEUE_ITEM_SIZE * size + QUEUE_HEADER_SIZE
}

fn calculate_orderbook_size(size: u64) -> u64 {
    ORDERBOOK_ITEM_SIZE * size + ORDERBOOK_HEADER_SIZE
}

fn calculate_event_queue_size(size: u64) -> u64 {
    EVENT_QUEUE_ITEM_SIZE * size + QUEUE_HEADER_SIZE
}

pub trait Signers {
    fn pubkeys(&self) -> Vec<Pubkey>;
    fn try_pubkeys(&self) -> Result<Vec<Pubkey>, SignerError>;
    fn sign_message(&self, message: &[u8]) -> Vec<Signature>;
    fn try_sign_message(&self, message: &[u8]) -> Result<Vec<Signature>, SignerError>;
    fn is_interactive(&self) -> bool;
}

macro_rules! default_keypairs_impl {
    () => {
        fn pubkeys(&self) -> Vec<Pubkey> {
            self.iter().map(|keypair| keypair.pubkey()).collect()
        }

        fn try_pubkeys(&self) -> Result<Vec<Pubkey>, SignerError> {
            let mut pubkeys = Vec::new();
            for keypair in self.iter() {
                pubkeys.push(keypair.try_pubkey()?);
            }
            Ok(pubkeys)
        }

        fn sign_message(&self, message: &[u8]) -> Vec<Signature> {
            self.iter()
                .map(|keypair| keypair.sign_message(message))
                .collect()
        }

        fn try_sign_message(&self, message: &[u8]) -> Result<Vec<Signature>, SignerError> {
            let mut signatures = Vec::new();
            for keypair in self.iter() {
                signatures.push(keypair.try_sign_message(message)?);
            }
            Ok(signatures)
        }

        fn is_interactive(&self) -> bool {
            self.iter().any(|s| s.is_interactive())
        }
    };
}

impl Signers for [&dyn Signer; 6] {
    default_keypairs_impl!();
}

#[allow(deprecated)]
pub fn open_market(
    project_dir: &str,
    rpc_client: &RpcClient,
    payer: &Keypair,
    project_config: &ProjectConfig,
    quote_mint: &str,
    event_queue_length: u64,
    request_queue_length: u64,
    orderbook_length: u64,
    cluster_type: ClusterType) {
    let bump = Bump::new();
    let token_keypair = Keypair::from_base58_string(&project_config.token_keypair);
    let mut tx1: Vec<Instruction> = vec![];

    let program_id_str: &str;
    match cluster_type {
        ClusterType::MainnetBeta => {
            program_id_str = SERUM_PROGRAM_ID;
        }
        ClusterType::Devnet => {
            program_id_str = SERUM_PROGRAM_DEV_ID;
        }
        _ => {
            unimplemented!();
        }
    }

    let program_id = Pubkey::from_str(program_id_str).unwrap();

    let market_keypair = Keypair::new();
    let bids_keypair = Keypair::new();
    let asks_keypair = Keypair::new();
    let request_keypair = Keypair::new();
    let event_keypair = Keypair::new();
    let base_vault_keypair = Keypair::new();
    let quote_vault_keypair = Keypair::new();

    let mut i = 0;
    let (vault_signer_nonce, vault_signer_pk) = loop {
        assert!(i < 100);
        if let Ok(pk) = openbook_dex::state::gen_vault_signer_key(i, &market_keypair.pubkey(), &program_id) {
            break (i, bump.alloc(pk));
        }
        i += 1;
    };

    // write keypair to disk
    let markets = MarketConfig {
        market_id: market_keypair.pubkey().to_string(),
        market_keypair: market_keypair.to_base58_string(),
        base_mint: token_keypair.pubkey().to_string(),
        quote_mint: quote_mint.to_string(),
        bids_keypair: bids_keypair.to_base58_string(),
        asks_keypair: asks_keypair.to_base58_string(),
        request_keypair: request_keypair.to_base58_string(),
        event_keypair: event_keypair.to_base58_string(),
        base_vault_keypair: base_vault_keypair.to_base58_string(),
        quote_vault_keypair: quote_vault_keypair.to_base58_string(),
        vault_signer_pk: vault_signer_pk.to_string(),
    };

    let market_config_file = format!("{}/market.yaml", project_dir);
    match std::fs::write(&market_config_file, serde_yaml::to_string(&markets).unwrap()) {
        Ok(_) => {
            info!("Market config updated");
        },
        Err(e) => {
            error!("Error updating market config: {:?}", e);
            return;
        }
    }

    tx1.push(
        solana_sdk::compute_budget::ComputeBudgetInstruction::set_compute_unit_limit(200000)
    );
    tx1.push(
        solana_sdk::compute_budget::ComputeBudgetInstruction::set_compute_unit_price(100000)
    );

    // Base Vault and Quote Vault Instructions
    tx1.push(
        solana_program::system_instruction::create_account(
            &payer.pubkey(),
            &base_vault_keypair.pubkey(),
            rpc_client.get_minimum_balance_for_rent_exemption(165).unwrap(),
            165,
            &spl_token::id(),
        )
    );

    tx1.push(
        solana_program::system_instruction::create_account(
            &payer.pubkey(),
            &quote_vault_keypair.pubkey(),
            rpc_client.get_minimum_balance_for_rent_exemption(165).unwrap(),
            165,
            &spl_token::id(),
        )
    );

    tx1.push(
        spl_token::instruction::initialize_account(
            &spl_token::id(),
            &base_vault_keypair.pubkey(),
            &token_keypair.pubkey(),
            &vault_signer_pk,
        ).unwrap()
    );

    tx1.push(
        spl_token::instruction::initialize_account(
            &spl_token::id(),
            &quote_vault_keypair.pubkey(),
            &Pubkey::from_str(&quote_mint).unwrap(),
            &vault_signer_pk,
        ).unwrap()
    );

    let mut tx2: Vec<Instruction> = vec![];

    tx2.push(
        solana_sdk::compute_budget::ComputeBudgetInstruction::set_compute_unit_limit(200000)
    );

    tx2.push(
        solana_sdk::compute_budget::ComputeBudgetInstruction::set_compute_unit_price(100000)
    );

    // tx2
    tx2.push(
        solana_program::system_instruction::create_account(
            &payer.pubkey(),
            &market_keypair.pubkey(),
            rpc_client.get_minimum_balance_for_rent_exemption(388).unwrap(),
            388,
            &program_id
        )
    );

    tx2.push(
        solana_program::system_instruction::create_account(
            &payer.pubkey(),
            &request_keypair.pubkey(),
            rpc_client.get_minimum_balance_for_rent_exemption(
                calculate_request_queue_size(request_queue_length) as usize
            ).unwrap(),
            calculate_request_queue_size(request_queue_length),
            &program_id
        )
    );

    tx2.push(
        solana_program::system_instruction::create_account(
            &payer.pubkey(),
            &event_keypair.pubkey(),
            rpc_client.get_minimum_balance_for_rent_exemption(
                calculate_event_queue_size(event_queue_length) as usize
            ).unwrap(),
            calculate_event_queue_size(event_queue_length),
            &program_id
        )
    );

    tx2.push(
        solana_program::system_instruction::create_account(
            &payer.pubkey(),
            &bids_keypair.pubkey(),
            rpc_client.get_minimum_balance_for_rent_exemption(
                calculate_orderbook_size(orderbook_length) as usize
            ).unwrap(),
            calculate_orderbook_size(orderbook_length),
            &program_id
        )
    );

    tx2.push(
        solana_program::system_instruction::create_account(
            &payer.pubkey(),
            &asks_keypair.pubkey(),
            rpc_client.get_minimum_balance_for_rent_exemption(
                calculate_orderbook_size(orderbook_length) as usize
            ).unwrap(),
            calculate_orderbook_size(orderbook_length),
            &program_id
        )
    );

    let lot_size = 1.0;
    let tick_size = 0.01;

    let coin_lot_size = f64::round(u64::pow(10, (project_config.decimal - 1) as u32) as f64 * lot_size) as u64;
    let pc_lot_size = f64::round(lot_size * LAMPORTS_PER_SOL as f64 * tick_size) as u64;

    let init_instruction = openbook_dex::instruction::initialize_market(
        &market_keypair.pubkey(),
        &program_id,
        &token_keypair.pubkey(),
        &Pubkey::from_str(&quote_mint).unwrap(),
        &base_vault_keypair.pubkey(),
        &quote_vault_keypair.pubkey(),
        None,
        None,
        None,
        &bids_keypair.pubkey(),
        &asks_keypair.pubkey(),
        &request_keypair.pubkey(),
        &event_keypair.pubkey(),
        coin_lot_size,
        pc_lot_size,
        vault_signer_nonce,
        0x1F4,
    ).unwrap();

    tx2.push(init_instruction);

    let transaction = Transaction::new_signed_with_payer(
        &tx1,
        Some(&payer.pubkey()),
        &[&payer, &base_vault_keypair, &quote_vault_keypair],
        rpc_client.get_recent_blockhash().unwrap().0
    );

    match rpc_client.send_and_confirm_transaction(&transaction) {
        Ok(s) => {
            info!("Create Vault Tx: {:?}", s);
        }
        Err(e) => {
            error!("Error: {:?}", e);
            return;
        }
    }

    let signers: [&dyn Signer; 6] = [&payer, &market_keypair, &request_keypair, &event_keypair, &bids_keypair, &asks_keypair];
    let mut transaction = Transaction::new_unsigned(
        Message::new(&tx2, Some(&payer.pubkey()))
    );

    transaction.message.recent_blockhash = rpc_client.get_recent_blockhash().unwrap().0;

    let positions = transaction.get_signing_keypair_positions(&signers.pubkeys()).unwrap();
    if positions.iter().any(|pos| pos.is_none()) {
        panic!("Some signer is missing from the transaction");
    }

    let positions: Vec<usize> = positions.iter().map(|pos| pos.unwrap()).collect();
    let signatures = signers.try_sign_message(&transaction.message_data()).unwrap();
    for i in 0..positions.len() {
        transaction.signatures[positions[i]] = signatures[i];
    };

    match rpc_client.send_transaction_with_config(&transaction, RpcSendTransactionConfig {
        skip_preflight: true,
        preflight_commitment: None,
        encoding: None,
        max_retries: None,
        min_context_slot: None,
    }) {
        Ok(s) => {
            info!("Create Market Tx: {:?}", s);
            info!("Market Id: {}", market_keypair.pubkey().to_string());
        }
        Err(e) => {
            error!("Error: {:?}", e);
            return;
        }
    }
}
