use std::io::{Cursor, Write};
use std::str::FromStr;
use borsh::{BorshDeserialize, BorshSerialize};
use bytes::Buf;
use reqwest::blocking::Response;
use serde_json::{json, Value};
use solana_account_decoder::UiAccountEncoding;
use solana_client::rpc_client::RpcClient;
use solana_client::rpc_config::{RpcAccountInfoConfig, RpcProgramAccountsConfig};
use solana_client::rpc_filter::{Memcmp, MemcmpEncodedBytes, RpcFilterType};
use solana_program::program_pack::{Pack, Sealed};
use solana_program::pubkey::Pubkey;
use solana_sdk::commitment_config::CommitmentConfig;
use solana_sdk::genesis_config::ClusterType;
use crate::dex::{openbook, raydium};
use crate::dex::raydium::error::{ParserError, RequestError};
use crate::dex::raydium::error::ParserError::{AccountDataDecodeError, AccountDataNotFound, AccountNotFound};
use crate::dex::raydium::error::RequestError::{GetLiquidityStateRequestError, GetMarketStateRequestError};

#[derive(Debug, BorshDeserialize, BorshSerialize)]
pub struct RemoveLiquidityLayout {
    pub instruction: u8,
    pub amount_in: u64,
}

#[derive(Debug, BorshDeserialize, BorshSerialize, PartialEq, Eq, Clone, Copy)]
pub struct RaydiumInstruction {
    pub instruction: u8,
    pub nonce: u8,
    pub open_time: u64,
    pub pc_amount: u64,
    pub coin_amount: u64,
}

#[derive(BorshSerialize, BorshDeserialize, Debug)]
#[allow(dead_code)]
pub struct AccountLayout {
    pub mint: Pubkey,
    pub owner: Pubkey,
    pub amount: u64,
    pub delegate_option: u32,
    pub delegate: Pubkey,
    pub state: u8,
    pub is_native_option: u32,
    pub is_native: u64,
    pub delegated_amount: u64,
    pub close_authority_option: u32,
    pub close_authority: Pubkey,
}

#[derive(Debug)]
pub struct SwapLayout {
    pub instruction: u8,
    pub amount_in: u64,
    pub min_amount_out: u64,
}

impl Sealed for SwapLayout {}

impl Pack for SwapLayout {
    const LEN: usize = 17; // 1 (u8) + 8 (i64) + 8 (i64)

    fn pack_into_slice(&self, dst: &mut [u8]) {
        let mut cursor = Cursor::new(dst);
        cursor.write_all(&[self.instruction]).unwrap();
        cursor.write_all(&self.amount_in.to_le_bytes()).unwrap();
        cursor.write_all(&self.min_amount_out.to_le_bytes()).unwrap();
    }

    fn unpack_from_slice(src: &[u8]) -> Result<Self, solana_program::program_error::ProgramError> {
        let mut cursor = Cursor::new(src);
        let instruction = cursor.get_u8();
        let amount_in = cursor.get_i64() as u64;
        let min_amount_out = cursor.get_i64() as u64;
        Ok(SwapLayout { instruction, amount_in, min_amount_out })
    }
}

#[derive(BorshSerialize, BorshDeserialize, Debug, Copy, Clone)]
pub struct LiquidityStateLayoutV4 {
    pub status: u64,
    pub nonce: u64,
    pub max_order: u64,
    pub depth: u64,
    pub base_decimal: u64,
    pub quote_decimal: u64,
    pub state: u64,
    pub reset_flag: u64,
    pub min_size: u64,
    pub vol_max_cut_ratio: u64,
    pub amount_wave_ratio: u64,
    pub base_lot_size: u64,
    pub quote_lot_size: u64,
    pub min_price_multiplier: u64,
    pub max_price_multiplier: u64,
    pub system_decimal_value: u64,
    pub min_separate_numerator: u64,
    pub min_separate_denominator: u64,
    pub trade_fee_numerator: u64,
    pub trade_fee_denominator: u64,
    pub pnl_numerator: u64,
    pub pnl_denominator: u64,
    pub swap_fee_numerator: u64,
    pub swap_fee_denominator: u64,
    pub base_need_take_pnl: u64,
    pub quote_need_take_pnl: u64,
    pub quote_total_pnl: u64,
    pub base_total_pnl: u64,
    pub pool_open_time: u64,
    pub punish_pc_amount: u64,
    pub punish_coin_amount: u64,
    pub orderbook_to_init_time: u64,
    pub swap_base_in_amount: u128,
    pub swap_quote_out_amount: u128,
    pub swap_base2_quote_fee: u64,
    pub swap_quote_in_amount: u128,
    pub swap_base_out_amount: u128,
    pub swap_quote2_base_fee: u64,
    // amm vault
    pub base_vault: Pubkey,
    pub quote_vault: Pubkey,
    // mint
    pub base_mint: Pubkey,
    pub quote_mint: Pubkey,
    pub lp_mint: Pubkey,
    // market
    pub open_orders: Pubkey,
    pub market_id: Pubkey,
    pub market_program_id: Pubkey,
    pub target_orders: Pubkey,
    pub withdraw_queue: Pubkey,
    pub lp_vault: Pubkey,
    pub owner: Pubkey,
    // true circulating supply without lock up
    pub lp_reserve: u64,
    pub padding: [u64; 3],
}

impl LiquidityStateLayoutV4 {
    pub fn get_config(base_mint: &Pubkey, quote_mint: &Pubkey) -> RpcProgramAccountsConfig {
        RpcProgramAccountsConfig {
            filters: Some(
                vec![
                    RpcFilterType::DataSize(752),
                    RpcFilterType::Memcmp(
                        Memcmp::new(
                            400, // baseMint
                            MemcmpEncodedBytes::Base58(base_mint.to_string()),
                        )
                    ),
                    RpcFilterType::Memcmp(
                        Memcmp::new(
                            432, // quoteMint
                            MemcmpEncodedBytes::Base58(quote_mint.to_string()),
                        )
                    ),
                ]),
            account_config: RpcAccountInfoConfig {
                encoding: Some(UiAccountEncoding::Base64),
                data_slice: None,
                commitment: Some(CommitmentConfig::processed()),
                min_context_slot: None,
            },
            ..Default::default()
        }
    }

    #[allow(unused)]
    pub async fn get_with_rpc(connection: &RpcClient,
                              base_mint: &Pubkey,
                              quote_mint: &Pubkey,
                              cluster_type: ClusterType)
                              -> Result<LiquidityStateLayoutV4, RequestError> {
        match cluster_type {
            ClusterType::Devnet => {
                Self::get_with_rpc_with_program_id(connection, base_mint, quote_mint, raydium::AMM_PROGRAM_DEV_ID).await
            }
            ClusterType::MainnetBeta => {
                Self::get_with_rpc_with_program_id(connection, base_mint, quote_mint, raydium::AMM_PROGRAM_ID).await
            }
            _ => {
                unimplemented!()
            }
        }
    }

    pub async fn get_with_rpc_with_program_id(connection: &RpcClient, base_mint: &Pubkey,
                                              quote_mint: &Pubkey, program_id: &str) -> Result<LiquidityStateLayoutV4, RequestError> {

        // amm_program_id is the address of the AMM program
        let amm_program_id =
            Pubkey::from_str(program_id).unwrap();

        let markets_v4_config = Self::get_config(&base_mint, &quote_mint);

        let (_, program_account) = match connection.get_program_accounts_with_config(
            &amm_program_id,
            markets_v4_config) {
            Ok(acc) => {
                if acc.is_empty() {
                    return Err(RequestError::AccountNotFound)
                }
                acc.first().unwrap().clone()
            },
            Err(e) => {
                return Err(RequestError::RpcError(e.to_string()))
            }
        };

        Ok(
            LiquidityStateLayoutV4::try_from_slice(&*program_account.data)
                .expect("Failed to deserialize LiquidityStateLayoutV4 data")
        )
    }

    pub async fn get_with_reqwest(api_url: &str,
                                  base_mint: &Pubkey,
                                  quote_mint: &Pubkey,
                                  cluster_type: ClusterType)
                                  -> Result<LiquidityStateLayoutV4, RequestError> {
        match cluster_type {
            ClusterType::Devnet => {
                Self::get_with_reqwest_with_program_id(api_url, base_mint, quote_mint, raydium::AMM_PROGRAM_DEV_ID).await
            }
            ClusterType::MainnetBeta => {
                Self::get_with_reqwest_with_program_id(api_url, base_mint, quote_mint, raydium::AMM_PROGRAM_ID).await
            }
            _ => {
                unimplemented!()
            }
        }
    }

    pub async fn get_with_reqwest_with_program_id<U: ToString + serde::Serialize>(api_url: &str, base_mint: &Pubkey, quote_mint: &Pubkey, program_id: U) -> Result<LiquidityStateLayoutV4, RequestError> {
        let markets_v4_config = Self::get_config(&base_mint, &quote_mint);
        let request = &json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "getProgramAccounts",
            "params": json!([program_id, markets_v4_config]),
        });

        tokio::task::block_in_place(|| {
            let client = reqwest::blocking::Client::new();
            let res = client.post(api_url)
                            .json(request)
                            .send()
                            .expect("failed to send getProgramAccounts request");

            account_data_parser(res)
                .map_err(|e| GetLiquidityStateRequestError(e.to_string()))
                .and_then(|decoded_account_data| {
                    Ok(LiquidityStateLayoutV4::try_from_slice(&decoded_account_data)
                        .expect("Failed to deserialize LiquidityStateLayoutV4 data"))
                })
        })
    }
}

#[allow(dead_code)]
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy)]
pub struct MarketStateLayoutV3 {
    pub reserved: [u8; 5],
    pub account_flags: [u8; 8],
    // Assuming accountFlagsLayout is a function that returns an 8-byte array
    pub own_address: Pubkey,
    pub vault_signer_nonce: u64,
    pub base_mint: Pubkey,
    pub quote_mint: Pubkey,
    pub base_vault: Pubkey,
    pub base_deposits_total: u64,
    pub base_fees_accrued: u64,
    pub quote_vault: Pubkey,
    pub quote_deposits_total: u64,
    pub quote_fees_accrued: u64,
    pub quote_dust_threshold: u64,
    pub request_queue: Pubkey,
    pub event_queue: Pubkey,
    pub bids: Pubkey,
    pub asks: Pubkey,
    pub base_lot_size: u64,
    pub quote_lot_size: u64,
    pub fee_rate_bps: u64,
    pub referrer_rebates_accrued: u64,
    pub reserved_2: [u8; 7],
}

impl MarketStateLayoutV3 {
    pub fn get_config(base_mint: &Pubkey, quote_mint: &Pubkey) -> RpcProgramAccountsConfig {
        RpcProgramAccountsConfig {
            filters: Some(vec![
                RpcFilterType::Memcmp(Memcmp::new(
                    53, // baseMint
                    MemcmpEncodedBytes::Base58(base_mint.to_string()),
                )),
                RpcFilterType::Memcmp(Memcmp::new(
                    85, // quoteMint
                    MemcmpEncodedBytes::Base58(quote_mint.to_string()),
                )),
                RpcFilterType::DataSize(388),
            ]),
            account_config: RpcAccountInfoConfig {
                encoding: Some(UiAccountEncoding::Base64),
                data_slice: None,
                commitment: Some(CommitmentConfig::confirmed()),
                min_context_slot: None,
            },
            ..Default::default()
        }
    }

    #[allow(unused)]
    pub async fn get_with_rpc(connection: &RpcClient,
                              base_mint: &Pubkey,
                              quote_mint: &Pubkey,
                              cluster_type: ClusterType)
                              -> Result<MarketStateLayoutV3, RequestError> {
        match cluster_type {
            ClusterType::Devnet => {
                Self::get_with_rpc_with_program_id(connection, base_mint, quote_mint, openbook::SERUM_PROGRAM_DEV_ID).await
            }
            ClusterType::MainnetBeta => {
                Self::get_with_rpc_with_program_id(connection, base_mint, quote_mint, openbook::SERUM_PROGRAM_ID).await
            }
            _ => {
                unimplemented!()
            }
        }
    }

    pub async fn get_with_rpc_with_program_id(connection: &RpcClient,
                                              base_mint: &Pubkey, quote_mint: &Pubkey, program_id: &str) -> Result<MarketStateLayoutV3, RequestError> {
        let markets_v3_config = Self::get_config(&base_mint, &quote_mint);

        let (_, program_account) = match connection.get_program_accounts_with_config(
            &Pubkey::from_str(program_id).unwrap(),
            markets_v3_config) {
            Ok(acc) => {
                if acc.is_empty() {
                    return Err(RequestError::AccountNotFound)
                }
                acc.first().unwrap().clone()
            },
            Err(e) => {
                return Err(RequestError::RpcError(e.to_string()))
            }
        };

        Ok(
            MarketStateLayoutV3::try_from_slice(&*program_account.data)
                .expect("Failed to deserialize MarketStateLayoutV3 data")
        )
    }

    pub async fn get_with_reqwest(api_url: &str,
                                  base_mint: &Pubkey,
                                  quote_mint: &Pubkey,
                                  cluster_type: ClusterType)
                                  -> Result<MarketStateLayoutV3, RequestError> {
        match cluster_type {
            ClusterType::Devnet => {
                Self::get_with_reqwest_with_program_id(api_url, base_mint, quote_mint, openbook::SERUM_PROGRAM_DEV_ID).await
            }
            ClusterType::MainnetBeta => {
                Self::get_with_reqwest_with_program_id(api_url, base_mint, quote_mint, openbook::SERUM_PROGRAM_ID).await
            }
            _ => {
                unimplemented!();
            }
        }
    }

    pub async fn get_with_reqwest_with_program_id<U: ToString + serde::Serialize>(api_url: &str,
                                                                                  base_mint: &Pubkey,
                                                                                  quote_mint: &Pubkey,
                                                                                  program_id: U) -> Result<MarketStateLayoutV3, RequestError> {
        let markets_v3_config = Self::get_config(&base_mint, &quote_mint);
        let request = &json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "getProgramAccounts",
            "params": json!([program_id, markets_v3_config]),
        });

        tokio::task::block_in_place(|| {
            let client = reqwest::blocking::Client::new();
            let res = client.post(api_url)
                            .json(request)
                            .send();

            account_data_parser(res.expect("failed to send getProgramAccounts request"))
                .map_err(|e| GetMarketStateRequestError(e.to_string()))
                .and_then(|decoded_account_data| {
                    Ok(MarketStateLayoutV3::try_from_slice(&decoded_account_data)
                        .expect("Failed to deserialize MarketStateLayoutV3 data"))
                })
        })
    }
}

#[allow(dead_code)]
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct PoolInfoLayout {
    pub instruction: u8,
    pub simulate_type: u8,
}

#[allow(deprecated)]
pub fn account_data_parser(res: Response) -> Result<Vec<u8>, ParserError> {
    let json_data: Value = serde_json::from_str(&*res.text()
                                                     .expect("Failed to get getProgramAccounts response"))
        .expect("failed to deserialize getProgramAccounts json response");

    let result = match json_data.get("result") {
        None => vec![],
        Some(r) => {
            r.as_array().unwrap().to_vec()
        }
    };

    let account = match result.first() {
        None => return Err(AccountNotFound),
        Some(a) => {
            match a.get("account") {
                None => return Err(AccountNotFound),
                Some(acc) => {
                    acc.as_object().unwrap().to_owned()
                }
            }
        }
    };

    let account_data_vector = match account.get("data") {
        None => return Err(AccountDataNotFound),
        Some(a) => {
            a.as_array()
             .unwrap()
             .to_vec()
        }
    };

    let account_data = match account_data_vector.first() {
        None => return Err(AccountDataNotFound),
        Some(a) => {
            a.as_str().unwrap()
        }
    };

    return match base64::decode(account_data) {
        Ok(d) => Ok(d),
        Err(e) => Err(AccountDataDecodeError(e.to_string()))
    };
}