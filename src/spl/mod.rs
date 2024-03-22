pub mod token;

use std::str::FromStr;
use borsh::{BorshDeserialize, BorshSerialize};
use log::{debug, info};
use mpl_token_metadata::types::DataV2;
use sha2::{Digest, Sha256};
use solana_client::rpc_client::{RpcClient};
use solana_client::rpc_request::TokenAccountsFilter;
use solana_program::instruction::{AccountMeta, Instruction};
use solana_program::program_pack::Pack;
use solana_program::pubkey::Pubkey;
use solana_program::system_instruction::SystemInstruction;
use solana_sdk::signature::{EncodableKey, Keypair, Signer};
use spl_token::instruction::AuthorityType;

pub const ASSOCIATED_TOKEN_PROGRAM_ID: &str = "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL";
pub const TOKEN_PROGRAM_ID: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";

pub fn get_associated_token_address(owner: &Pubkey, mint: &Pubkey) -> Pubkey {
    let (key, _) = Pubkey::find_program_address(
        &[
            owner.as_ref(),
            Pubkey::from_str(TOKEN_PROGRAM_ID).unwrap().as_ref(),
            mint.as_ref(),
        ],
        &Pubkey::from_str(ASSOCIATED_TOKEN_PROGRAM_ID).unwrap(),
    );
    key
}

pub fn create_token_instruction<U: ToString>(client: &RpcClient,
                                             payer: &Keypair,
                                             token_keypair: &Keypair,
                                             token_name: U,
                                             token_symbol: U,
                                             token_uri: U,
                                             mint_amount: u64,
                                             decimal: u8,
                                             freeze: bool) -> Vec<Instruction> {
    let lamports = client.get_minimum_balance_for_rent_exemption(spl_token::state::Mint::LEN).unwrap();
    let mut instructions: Vec<Instruction> = vec![];

    instructions.push(
        solana_sdk::compute_budget::ComputeBudgetInstruction::set_compute_unit_limit(100000)
    );

    instructions.push(
        solana_sdk::compute_budget::ComputeBudgetInstruction::set_compute_unit_price(100000)
    );

    instructions.push(
        solana_program::system_instruction::create_account(
            &payer.pubkey(),
            &token_keypair.pubkey(),
            lamports,
            spl_token::state::Mint::LEN as u64,
            &spl_token::id(),
        )
    );

    let token_ata = get_associated_token_address(
        &payer.pubkey(), &token_keypair.pubkey(),
    );

    let associated_token_instruction = create_associated_token_account(
        &payer.pubkey(), &payer.pubkey(), &token_keypair.pubkey(),
    );

    let metadata_args = mpl_token_metadata::instructions::CreateMetadataAccountV3InstructionArgs {
        data: DataV2 {
            name: token_name.to_string(),
            symbol: token_symbol.to_string(),
            uri: token_uri.to_string(),
            seller_fee_basis_points: 0,
            creators: None,
            collection: None,
            uses: None,
        },
        is_mutable: true,
        collection_details: None,
    };

    let (metadata_pda, _) = Pubkey::find_program_address(
        &[
            "metadata".as_bytes(),
            mpl_token_metadata::programs::MPL_TOKEN_METADATA_ID.as_ref(),
            token_keypair.pubkey().as_ref(),
        ],
        &mpl_token_metadata::programs::MPL_TOKEN_METADATA_ID,
    );

    let meta_data = mpl_token_metadata::instructions::CreateMetadataAccountV3 {
        metadata: metadata_pda,
        mint: token_keypair.pubkey(),
        mint_authority: payer.pubkey(),
        payer: payer.pubkey(),
        update_authority: (payer.pubkey(), true),
        system_program: solana_program::system_program::id(),
        rent: None,
    };

    instructions.push(
        spl_token::instruction::initialize_mint(
            &spl_token::id(),
            &token_keypair.pubkey(),
            &payer.pubkey(),
            None,
            decimal,
        )
            .unwrap()
    );

    instructions.push(
        meta_data.instruction(metadata_args)
    );

    instructions.push(
        associated_token_instruction
    );

    instructions.push(
        spl_token::instruction::mint_to(
            &spl_token::id(),
            &token_keypair.pubkey(),
            &token_ata,
            &payer.pubkey(),
            &[],
            mint_amount * u64::pow(10, decimal as u32),
        ).unwrap()
    );


    if freeze {
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
    }

    return instructions;
}

#[allow(unused)]
pub fn get_token_account_unchecked(owner_pubkey: &Pubkey,
                                   payer_pubkey: &Pubkey,
                                   mint_pubkey: &Pubkey) -> (Pubkey, Instruction) {
    let swap_associated_token_address = get_associated_token_address(
        owner_pubkey, mint_pubkey,
    );

    let swap_token_account_instructions = create_associated_token_account(
        payer_pubkey, owner_pubkey, mint_pubkey,
    );

    return (swap_associated_token_address, swap_token_account_instructions);
}

pub fn get_token_account(connection: &RpcClient,
                         owner_pubkey: &Pubkey,
                         payer_pubkey: &Pubkey,
                         mint_pubkey: &Pubkey) -> (Pubkey, Option<Instruction>) {
    debug!("getting token account for {:?}", mint_pubkey);
    let account = connection.get_token_accounts_by_owner(
        &owner_pubkey,
        TokenAccountsFilter::Mint(*mint_pubkey),
    ).unwrap();

    if account.is_empty() {
        debug!("token account not found, creating one");

        let swap_associated_token_address = get_associated_token_address(
            owner_pubkey, mint_pubkey,
        );

        let swap_token_account_instructions = create_associated_token_account(
            payer_pubkey, owner_pubkey, mint_pubkey,
        );

        return (swap_associated_token_address, Option::from(swap_token_account_instructions));
    }

    let account = account.first();
    let account_pubkey_str = account.unwrap();
    debug!("token account found: {}", account_pubkey_str.pubkey.clone());

    (Pubkey::from_str(account_pubkey_str.pubkey.as_str()).unwrap(), None)
}


pub fn wait_token_account(connection: &RpcClient,
                         owner_pubkey: &Pubkey,
                         payer_pubkey: &Pubkey,
                         mint_pubkey: &Pubkey) -> (Pubkey, Option<Instruction>) {
    debug!("getting token account for {:?}", mint_pubkey);
    let mut account = connection.get_token_accounts_by_owner(
        &owner_pubkey,
        TokenAccountsFilter::Mint(*mint_pubkey),
    );

    loop {
        if account.is_err() {
            account = connection.get_token_accounts_by_owner(
                &owner_pubkey,
                TokenAccountsFilter::Mint(*mint_pubkey),
            );
            continue;
        }

        if account.is_ok() {
            break;
        }
    }

    let account = account.unwrap();
    if account.is_empty() {
        debug!("token account not found, creating one");

        let swap_associated_token_address = get_associated_token_address(
            owner_pubkey, mint_pubkey,
        );

        let swap_token_account_instructions = create_associated_token_account(
            payer_pubkey, owner_pubkey, mint_pubkey,
        );

        return (swap_associated_token_address, Option::from(swap_token_account_instructions));
    }

    let account = account.first();
    let account_pubkey_str = account.unwrap();
    debug!("token account found: {}", account_pubkey_str.pubkey.clone());

    (Pubkey::from_str(account_pubkey_str.pubkey.as_str()).unwrap(), None)
}


#[derive(Debug, Clone, BorshSerialize, BorshDeserialize)]
pub struct InitializeAccount {
    pub instruction: u8
}


pub fn create_account_with_seed(
    from_pubkey: &Pubkey,
    to_pubkey: &Pubkey, // must match create_with_seed(base, seed, owner)
    base: &Pubkey,
    seed: &str,
    lamports: u64,
    space: u64,
    owner: &Pubkey,
) -> Instruction {
    let account_metas = vec![
        AccountMeta::new(*from_pubkey, true),
        AccountMeta::new(*to_pubkey, false),
    ];

    Instruction::new_with_bincode(
        solana_program::system_program::id(),
        &SystemInstruction::CreateAccountWithSeed {
            base: *base,
            seed: seed.to_string(),
            lamports,
            space,
            owner: *owner,
        },
        account_metas,
    )
}

pub fn create_initialize_account_instruction(program_id: &Pubkey, mint: &Pubkey, account: &Pubkey, owner: &Pubkey) -> Instruction {
    let metadata = vec![
        AccountMeta::new(*account, false),
        AccountMeta::new_readonly(*mint, false),
        AccountMeta::new(*owner, false),
        AccountMeta::new_readonly(solana_program::sysvar::rent::id(), false),
    ];

    let data = InitializeAccount { instruction: 1 };

    Instruction::new_with_borsh(*program_id, &data, metadata)
}

#[allow(deprecated)]
pub fn generate_pubkey(from_public_key: &Pubkey, program_id: &Pubkey, project_dir: &str) -> (Pubkey, String) {
    let keypair = Keypair::new();

    match keypair.write_to_file(format!("{}/generated-pubkey-{}.json", project_dir, keypair.pubkey().to_string())) {
        Ok(_) => {
            info!("generated keypair written to file");
        }
        Err(_) => {
            info!("failed to write generated keypair to file, trying to write to current directory");
            keypair.write_to_file(format!("generated-pubkey-{}.json", keypair.pubkey().to_string())).unwrap();
         }
    }
    info!("generated keypair: {:?}", keypair.pubkey().to_string());

    let seed = &keypair.pubkey().to_string()[0..32];

    let mut buffer: Vec<u8> = Vec::new();
    buffer.extend_from_slice(&from_public_key.to_bytes());
    buffer.extend_from_slice(seed.as_bytes());
    buffer.extend_from_slice(&program_id.to_bytes());

    let mut hasher = Sha256::new();
    sha2::digest::Update::update(&mut hasher, buffer.as_slice());
    let result = &hasher.finalize()[..];

    return (Pubkey::new(result), seed.to_string());
}


pub fn create_associated_token_account(
    payer: &Pubkey,
    owner: &Pubkey,
    mint: &Pubkey,
) -> Instruction {
    let associated_token_address = get_associated_token_address(owner, mint);

    Instruction {
        program_id: Pubkey::from_str(ASSOCIATED_TOKEN_PROGRAM_ID).unwrap(),
        accounts: vec![
            AccountMeta::new(*payer, true),
            AccountMeta::new(associated_token_address, false),
            AccountMeta::new_readonly(*owner, false),
            AccountMeta::new_readonly(*mint, false),
            AccountMeta::new_readonly(solana_program::system_program::id(), false),
            AccountMeta::new_readonly(Pubkey::from_str(TOKEN_PROGRAM_ID).unwrap(), false),
            AccountMeta::new_readonly(solana_program::sysvar::rent::id(), false),
        ],
        data: vec![],
    }
}

