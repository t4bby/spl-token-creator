use colored::Colorize;
use log::{debug, error, info};
use solana_client::rpc_client::{RpcClient};
use solana_client::rpc_config::RpcSendTransactionConfig;
use solana_program::instruction::Instruction;
use solana_program::native_token::{lamports_to_sol, sol_to_lamports};
use solana_program::pubkey::Pubkey;
use solana_sdk::commitment_config::{CommitmentConfig, CommitmentLevel};
use solana_sdk::signature::Keypair;
use solana_sdk::signer::Signer;
use solana_sdk::transaction::Transaction;
use spl_token::instruction::AuthorityType;
use crate::cli::config::ProjectConfig;
use crate::spl;


#[derive(Debug)]
pub struct MyKeypair(Keypair);

impl Clone for MyKeypair {
    fn clone(&self) -> Self {
        let bytes = self.0.to_bytes();
        let clone = Keypair::from_bytes(&bytes).unwrap();
        Self(clone)
    }
}

#[derive(Debug, Clone)]
pub struct WalletInformation {
    pub wallet: String,
    pub wsol_account: Pubkey,
    pub token_account: Pubkey,
    pub balance: u64,
    pub create_token_account_instruction: Option<Instruction>,
}

pub fn get_wallet_token_information(rpc_client: &RpcClient, wallet_bs58: &str, wsol_account_pub: &Pubkey, mint: &Pubkey) -> WalletInformation {
    let wallet = Keypair::from_base58_string(wallet_bs58);
    debug!("Wallet Pubkey: {}", wallet.pubkey().to_string());

    let (token_account, create_token);
    if *mint != spl_token::native_mint::id() {
        (token_account, create_token) = spl::get_token_account(
            rpc_client,
            &wallet.pubkey(),
            &wallet.pubkey(),
            mint
        );

        if create_token.is_some() {
            return WalletInformation {
                wallet: wallet_bs58.to_string(),
                wsol_account: *wsol_account_pub,
                token_account,
                balance: 0,
                create_token_account_instruction: None,
            };
        }
    } else {
        token_account = *wsol_account_pub;
    }

    let mut balance: u64 = 0u64;
    let mut tries: u64 = 0u64;
    loop {
        if tries >= 5 {
            break;
        }
        let b = match rpc_client.get_token_account_balance(&token_account) {
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

    if *mint != spl_token::native_mint::id() {
        debug!("Token account: {:?}", token_account);
    }

    debug!("WSOL account: {:?}", wsol_account_pub);
    debug!("Balance: {:?}", balance);

    WalletInformation {
        wallet: wallet_bs58.to_string(),
        wsol_account: *wsol_account_pub,
        token_account,
        balance,
        create_token_account_instruction: None,
    }
}

#[allow(deprecated)]
pub fn revoke_mint_authority(rpc_client: &RpcClient,
                             payer: &Keypair,
                             project_config: &ProjectConfig) {
    let token_keypair = Keypair::from_base58_string(&project_config.token_keypair);

    let mut instructions: Vec<Instruction> = vec![];
    instructions.push(
        solana_sdk::compute_budget::ComputeBudgetInstruction::set_compute_unit_limit(200_000)
    );

    instructions.push(
        solana_sdk::compute_budget::ComputeBudgetInstruction::set_compute_unit_price(200_000)
    );

    instructions.push(
        spl_token::instruction::set_authority(
            &spl_token::id(),
            &token_keypair.pubkey(),
            None,
            AuthorityType::MintTokens,
            &payer.pubkey(),
            &[],
        ).unwrap()
    );

    let transaction = Transaction::new_signed_with_payer(
        &instructions,
        Some(&payer.pubkey()),
        &[&payer],
        rpc_client.get_recent_blockhash().unwrap().0
    );

    match rpc_client.send_and_confirm_transaction_with_spinner_and_commitment(&transaction,
                                                                              CommitmentConfig::confirmed()) {
        Ok(s) => {
            info!("Revoke Mint Authority Tx: {}", s.to_string().bold().green());
        }
        Err(e) => {
            panic!("Error: {:?}", e);
        }
    }
}

#[allow(deprecated)]
pub fn create(rpc_client: &RpcClient,
              payer: &Keypair,
              project_config: &ProjectConfig,
              freeze: bool) {
    let token_keypair = Keypair::from_base58_string(&project_config.token_keypair);

    let instructions = spl::create_token_instruction(
        &rpc_client,
        &payer,
        &token_keypair,
        &project_config.name,
        &project_config.symbol,
        &project_config.metadata_uri,
        project_config.mint_amount,
        project_config.decimal,
        freeze
    );

    // send transaction
    let transaction = Transaction::new_signed_with_payer(
        &instructions,
        Some(&payer.pubkey()),
        &[&payer, &token_keypair],
        rpc_client.get_recent_blockhash().unwrap().0
    );

    match rpc_client.send_and_confirm_transaction_with_spinner_and_commitment(&transaction,
                                                                              CommitmentConfig::confirmed()) {
        Ok(a) => {
            info!("Token created");
            info!("Token address: {}", token_keypair.pubkey().to_string().bold().green());
            info!("Token Creation Tx: {}", a.to_string().bold().green());
        }
        Err(e) => {
            panic!("Error creating token: {:?}", e);
        }
    }
}

#[allow(deprecated)]
pub fn airdrop(rpc_client: &RpcClient, payer: &Keypair, project_dir: &str,
               project_config: &mut ProjectConfig, percent: f64, confirm: bool) {
    let token_keypair = Keypair::from_base58_string(&project_config.token_keypair);

    let balance = rpc_client.get_balance(&payer.pubkey()).unwrap();
    info!("Wallet Balance: {:?} SOL", lamports_to_sol(balance));

    let balance_needed = 0.02f64 * project_config.wallets.len() as f64;
    if lamports_to_sol(balance) < balance_needed {
        error!("Insufficient balance for airdrop. Requires at least {} SOL", balance_needed);
        return;
    }

    let amount = project_config.mint_amount as f64 * (percent / 100f64);
    info!("Airdrop amount: {:?}", amount);

    let shared_amount = amount as u64 / project_config.wallets.len() as u64;
    info!("Shared amount: {:?}", shared_amount);

    let mut instructions: Vec<Instruction> = vec![];

    let (payer_token_account, _) = spl::get_token_account(
        rpc_client, &payer.pubkey(), &payer.pubkey(), &token_keypair.pubkey()
    );

    let wallets: Vec<Keypair> = project_config.wallets.iter().map(|w| {
        Keypair::from_base58_string(w)
    }).collect();

    for i in 0..wallets.len() {
        let wallet = &wallets[i];

        info!("Airdrop to wallet: {:?}", wallet.pubkey());

        let (token_account, create_token_account_instruction) = spl::get_token_account(
            rpc_client, &wallet.pubkey(), &payer.pubkey(), &token_keypair.pubkey()
        );

        if create_token_account_instruction.is_some() {
            instructions.push(
                create_token_account_instruction.unwrap()
            );
        }

        // if wsol account is not available send solana
        if project_config.wsol_wallets.len() == 0 {
            instructions.push(
                solana_program::system_instruction::transfer(
                    &payer.pubkey(),
                    &wallet.pubkey(),
                    sol_to_lamports(0.02),
                )
            );
        }

        instructions.push(
            spl_token::instruction::transfer(
                &spl_token::id(),
                &payer_token_account,
                &token_account,
                &payer.pubkey(),
                &[&payer.pubkey()],
                shared_amount * u64::pow(10, project_config.decimal as u32),
            ).unwrap()
        );
    }

    // split transaction into 7
    let instruction_chunks: Vec<&[Instruction]> = instructions.chunks(9 * 2).collect();

    for chunks in instruction_chunks {
        let mut new_instruction = vec![];
        new_instruction.push(
            solana_sdk::compute_budget::ComputeBudgetInstruction::set_compute_unit_limit(200_000)
        );
        new_instruction.push(
            solana_sdk::compute_budget::ComputeBudgetInstruction::set_compute_unit_price(200_000)
        );
        new_instruction.extend_from_slice(chunks);

        let transaction = Transaction::new_signed_with_payer(
            &new_instruction,
            Some(&payer.pubkey()),
            &[&payer],
            rpc_client.get_recent_blockhash().unwrap().0
        );

        match rpc_client.send_and_confirm_transaction_with_spinner_and_commitment(&transaction,
                                                                                  CommitmentConfig::confirmed()) {
            Ok(s) => {
                info!("Airdrop Tx: {}", s.to_string().bold().green());
            }
            Err(e) => {
                panic!("Error: {:?}", e);
            }
        }
    }

    // if WSOL account is not available, create one
    if project_config.wsol_wallets.len() == 0 {
        for wallet in wallets {
            let (_, wsol_keypair) = create_wsol_account(
                &rpc_client,
                &wallet,
                0.015,
                confirm
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


#[allow(deprecated)]
pub fn close_wsol_account(
    rpc_client: &RpcClient,
    wallet: &Keypair,
    wsol_account: &Pubkey,
) {
    let instructions: Vec<Instruction> = vec![
        spl_token::instruction::close_account(
            &spl_token::id(),
            &wsol_account,
            &wallet.pubkey(),
            &wallet.pubkey(),
            &[],
        ).unwrap()
    ];

    let transaction = Transaction::new_signed_with_payer(
        &instructions,
        Some(&wallet.pubkey()),
        &[&wallet],
        rpc_client.get_recent_blockhash().unwrap().0
    );

    match rpc_client.send_and_confirm_transaction_with_spinner_and_commitment(&transaction,
                                                                              CommitmentConfig::confirmed()) {
        Ok(s) => {
            info!("Close WSOL Account Tx: {}", s.to_string().bold().green());
        }
        Err(e) => {
            panic!("Error: {:?}", e);
        }
    }
}

#[allow(deprecated)]
pub fn create_wsol_account(
    rpc_client: &RpcClient,
    wallet: &Keypair,
    transfer_amount: f64,
    confirm: bool
) -> (Pubkey, Keypair) {
    info!("Creating WSOL account");

    let (instructions, wsol_keypair)
        = create_wsol_account_instruction(
        &wallet.pubkey(),
        &wallet.pubkey(),
        sol_to_lamports(transfer_amount),
        rpc_client.get_minimum_balance_for_rent_exemption(164).unwrap()
    );

    let transaction = Transaction::new_signed_with_payer(
        &instructions,
        Some(&wallet.pubkey()),
        &[&wallet, &wsol_keypair],
        rpc_client.get_recent_blockhash().unwrap().0
    );

    info!("Sending transaction");
    if confirm {
        match rpc_client.send_and_confirm_transaction_with_spinner_and_commitment(&transaction,
                                                                                  CommitmentConfig::confirmed()) {
            Ok(s) => {
                info!("WSOL Account: {}", wsol_keypair.pubkey().to_string().bold().green());
                info!("WSOL Account Creation Tx: {}", s.to_string().bold().green());
            }
            Err(e) => {
                panic!("Error: {:?}", e);
            }
        }
    } else {
        match rpc_client.send_transaction_with_config(&transaction,
                                                      RpcSendTransactionConfig {
                                                          skip_preflight: false,
                                                          preflight_commitment: Some(CommitmentLevel::Confirmed),
                                                          encoding: None,
                                                          max_retries: None,
                                                          min_context_slot: None,
                                                      }) {
            Ok(s) => {
                info!("WSOL Account: {}", wsol_keypair.pubkey().to_string().bold().green());
                info!("WSOL Account Creation Tx: {}", s.to_string().bold().green());
            }
            Err(e) => {
                panic!("Error: {:?}", e);
            }
        }
    }


    (wsol_keypair.pubkey(), wsol_keypair.insecure_clone())
}

pub fn create_wsol_account_instruction(
    owner: &Pubkey,
    payer: &Pubkey,
    amount: u64,
    balance_needed: u64
) -> (Vec<Instruction>, Keypair) {
    let new_keypair = Keypair::new();
    let create_account_instruction = solana_program::system_instruction::create_account(
        payer,
        &new_keypair.pubkey(),
        balance_needed,
        165, // AccountLayout.span
        &spl_token::id(),
    );

    let transfer_instruction = solana_program::system_instruction::transfer(payer,
                                                                            &new_keypair.pubkey(),
                                                                            amount);

    let initialize_account_instruction = spl_token::instruction::initialize_account(
        &spl_token::id(),
        &new_keypair.pubkey(),
        &spl_token::native_mint::id(),
        &owner,
    ).unwrap();

    (vec![solana_sdk::compute_budget::ComputeBudgetInstruction::set_compute_unit_price(100000),
          solana_sdk::compute_budget::ComputeBudgetInstruction::set_compute_unit_limit(200000),
          create_account_instruction,
          transfer_instruction,
          initialize_account_instruction], new_keypair)
}

#[allow(deprecated)]
pub fn burn(rpc_client: &RpcClient,
            payer: &Keypair,
            burn_account: &Keypair,
            token_mint: &Pubkey,
            percent: f64) {
    let mut instructions: Vec<Instruction> = vec![];

    let (ata_token_account, _) = spl::get_token_account(
        rpc_client, &burn_account.pubkey(), &payer.pubkey(), &token_mint
    );

    let mut balance: u64 = 0u64;
    let b = rpc_client.get_token_account_balance(&ata_token_account);
    if b.is_ok() {
        let b = b.unwrap();
        let decimal = b.decimals;
        balance = (b.ui_amount.unwrap() * 10f64.powf(decimal as f64)) as u64;
    }

    instructions.push(
        spl_token::instruction::burn(
            &spl_token::id(),
            &ata_token_account,
            &token_mint,
            &payer.pubkey(),
            &[&payer.pubkey(), &burn_account.pubkey()],
            (balance as f64 * (percent / 100f64)) as u64,
        ).unwrap()
    );

    let transaction = Transaction::new_signed_with_payer(
        &instructions,
        Some(&payer.pubkey()),
        &[&payer, &burn_account],
        rpc_client.get_recent_blockhash().unwrap().0
    );

    match rpc_client.send_and_confirm_transaction_with_spinner_and_commitment(&transaction,
                                                                              CommitmentConfig::confirmed()) {
        Ok(s) => {
            info!("Burn Tx: {}", s.to_string().bold().green());
        }
        Err(e) => {
            panic!("Error: {:?}", e);
        }
    }
}

#[allow(deprecated)]
pub fn send(rpc_client: &RpcClient,
            destination: &Pubkey,
            project_config: &ProjectConfig) {
    let mut wallet_information: Vec<WalletInformation> = vec![];

    for i in 0..project_config.wallets.len() {
        let wsol_wallet = Keypair::from_base58_string(&project_config.wsol_wallets[i]);
        wallet_information.push(
            get_wallet_token_information(
                &rpc_client,
                &project_config.wallets[i],
                &wsol_wallet.pubkey(),
                &spl_token::native_mint::id(),
            )
        );
    }

    for wallet in wallet_information.iter() {
        let wallet_keypair = Keypair::from_base58_string(&wallet.wallet);
        let mut instructions: Vec<Instruction> = vec![];

        let mut balance = wallet.balance.clone();
        if balance != 0u64 {
            instructions.push(
                spl_token::instruction::close_account(
                    &spl_token::id(),
                    &wallet.wsol_account,
                    &wallet_keypair.pubkey(),
                    &wallet_keypair.pubkey(),
                    &[],
                ).unwrap()
            );
        }

        if wallet.balance == 0u64 {
            // get main sol account balance
            balance = rpc_client.get_balance(&wallet_keypair.pubkey()).unwrap();
            info!("Wallet: {:?}", &wallet_keypair.pubkey());
            info!("Wallet Balance: {:?} SOL", lamports_to_sol(balance));
        }

        instructions.push(
            solana_program::system_instruction::transfer(
                &wallet_keypair.pubkey(),
                &destination,
                balance
            )
        );

        let transaction = Transaction::new_signed_with_payer(
            &instructions,
            Some(&wallet_keypair.pubkey()),
            &[&wallet_keypair],
            rpc_client.get_recent_blockhash().unwrap().0
        );

        info!("Sending {:?} SOL to {:?} from {:?}",
            lamports_to_sol(wallet.balance),
            destination, wallet_keypair.pubkey());

        match rpc_client.send_transaction_with_config(&transaction, RpcSendTransactionConfig {
            skip_preflight: false,
            preflight_commitment: Some(CommitmentLevel::Confirmed),
            encoding: None,
            max_retries: None,
            min_context_slot: None,
        }) {
            Ok(s) => {
                info!("Send Tx: {}", s.to_string().bold().green());
            }
            Err(e) => {
                error!("Error: {:?}", e);
            }
        }
    }
}