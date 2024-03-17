use std::str::FromStr;
use solana_client::rpc_client::RpcClient;
use solana_program::pubkey::{Pubkey};
use futures::join;
use solana_sdk::genesis_config::ClusterType;
use crate::dex::raydium::error::PoolError;
use crate::dex::raydium::layout::{LiquidityStateLayoutV4, MarketStateLayoutV3};
use crate::dex;

#[derive(Debug, Clone)]
pub struct LiquidityPoolInfo {
    pub id: Pubkey,
    pub authority: Pubkey,
    pub open_orders: Pubkey,
    pub target_orders: Pubkey,
    pub base_vault: Pubkey,
    pub quote_vault: Pubkey,
    pub market_program_id: Pubkey,
    pub market_id: Pubkey,
    pub bids: Pubkey,
    pub asks: Pubkey,
    pub event_queue: Pubkey,
    pub market_base_vault: Pubkey,
    pub market_quote_vault: Pubkey,
    pub market_authority: Pubkey,
    pub lp_mint: Pubkey,
    // optional
    pub liquidity_state: LiquidityStateLayoutV4,
    pub market_state: MarketStateLayoutV3,
}

impl LiquidityPoolInfo {
    pub fn build(liquidity_state: LiquidityStateLayoutV4, market_state: MarketStateLayoutV3, cluster_type: ClusterType)
                 -> Result<LiquidityPoolInfo, PoolError> {
        let mut amm_program_id = Pubkey::default();
        let mut serum_program_id = Pubkey::default();

        match cluster_type {
            ClusterType::MainnetBeta => {
                amm_program_id = Pubkey::from_str(dex::raydium::AMM_PROGRAM_ID).unwrap();
                serum_program_id = Pubkey::from_str(dex::openbook::SERUM_PROGRAM_ID).unwrap();
            },
            ClusterType::Devnet => {
                amm_program_id = Pubkey::from_str(dex::raydium::AMM_PROGRAM_DEV_ID).unwrap();
                serum_program_id = Pubkey::from_str(dex::openbook::SERUM_PROGRAM_DEV_ID).unwrap();
            },
            _ => {
                unimplemented!()
            }
        }

        Ok(LiquidityPoolInfo {
            id: Self::get_associated_id(amm_program_id, liquidity_state.market_id).0,
            authority: Self::get_associated_authority(amm_program_id).0,
            open_orders: liquidity_state.open_orders,
            target_orders: liquidity_state.target_orders,
            base_vault: liquidity_state.base_vault,
            quote_vault: liquidity_state.quote_vault,
            market_program_id: liquidity_state.market_program_id,
            market_id: liquidity_state.market_id,
            bids: market_state.bids,
            asks: market_state.asks,
            event_queue: market_state.event_queue,
            market_base_vault: market_state.base_vault,
            market_quote_vault: market_state.quote_vault,
            market_authority: match Self::get_market_authority(&serum_program_id,
                                                               &liquidity_state.market_id) {
                Some(a) => a,
                None => {
                    return Err(PoolError::GetMarketAuthorityError)
                }
            },
            lp_mint: liquidity_state.lp_mint,
            liquidity_state,
            market_state,
        })
    }
    pub async fn build_with_rpc(
        connection: &RpcClient,
        base_mint: &str,
        quote_mint: &str,
        cluster_type: ClusterType
    ) -> Result<LiquidityPoolInfo, PoolError> {
        match cluster_type {
            ClusterType::MainnetBeta => {
                Self::build_with_rpc_with_program_id(
                    connection,
                    base_mint,
                    quote_mint,
                    dex::raydium::AMM_PROGRAM_ID,
                    dex::openbook::SERUM_PROGRAM_ID,
                    cluster_type
                ).await
            }
            ClusterType::Devnet => {
                Self::build_with_rpc_with_program_id(
                    connection,
                    base_mint,
                    quote_mint,
                    dex::raydium::AMM_PROGRAM_DEV_ID,
                    dex::openbook::SERUM_PROGRAM_DEV_ID,
                    cluster_type
                ).await
            }
            _ => {
                unimplemented!()
            }
        }
    }

    pub async fn build_with_rpc_with_program_id(connection: &RpcClient,
                                                base_mint: &str,
                                                quote_mint: &str,
                                                liquidity_program_id: &str,
                                                market_state_program_id: &str,
                                                cluster_type: ClusterType)
                                                -> Result<LiquidityPoolInfo, PoolError> {
        let base_mint_pub = Pubkey::from_str(base_mint).unwrap();
        let quote_mint_pub = Pubkey::from_str(quote_mint).unwrap();

        let (liquidity_state, market_state) = join!(
            LiquidityStateLayoutV4::get_with_rpc_with_program_id(connection, &base_mint_pub, &quote_mint_pub, liquidity_program_id),
            MarketStateLayoutV3::get_with_rpc_with_program_id(connection, &base_mint_pub, &quote_mint_pub, market_state_program_id));

        let liquidity_state = match liquidity_state {
            Ok(a) => a,
            Err(_) => {
                return Err(PoolError::GetLiquidityStateError)
            }
        };

        let market_state = match market_state {
            Ok(a) => a,
            Err(_) => {
                return Err(PoolError::GetMarketStateError)
            }
        };

        Ok(
            match Self::build(
                liquidity_state,
                market_state,
                cluster_type
            ) {
                Ok(a) => a,
                Err(_) => {
                    return Err(PoolError::BuildLiquidityInfoError)
                }
            }
        )
    }

    pub async fn build_with_request(
        api_url: &str,
        base_mint: &str,
        quote_mint: &str,
        cluster_type: ClusterType
    ) -> Result<LiquidityPoolInfo, PoolError> {
        match cluster_type {
            ClusterType::MainnetBeta => {
                Self::build_with_request_with_program_id(
                    api_url,
                    base_mint,
                    quote_mint,
                    dex::raydium::AMM_PROGRAM_ID,
                    dex::openbook::SERUM_PROGRAM_ID,
                    cluster_type
                ).await
            }
            ClusterType::Devnet => {
                Self::build_with_request_with_program_id(
                    api_url,
                    base_mint,
                    quote_mint,
                    dex::raydium::AMM_PROGRAM_DEV_ID,
                    dex::openbook::SERUM_PROGRAM_DEV_ID,
                    cluster_type
                ).await
            }
            _ => {
                unimplemented!()
            }
        }
    }
    pub async fn build_with_request_with_program_id(api_url: &str,
                                                    base_mint: &str,
                                                    quote_mint: &str,
                                                    liquidity_program_id: &str,
                                                    market_state_program_id: &str,
                                                    cluster_type: ClusterType
    ) -> Result<LiquidityPoolInfo, PoolError> {
        let base_mint_pub = Pubkey::from_str(base_mint).unwrap();
        let quote_mint_pub = Pubkey::from_str(quote_mint).unwrap();

        let (liquidity_state, market_state) = join!(
            LiquidityStateLayoutV4::get_with_reqwest_with_program_id(api_url, &base_mint_pub, &quote_mint_pub, liquidity_program_id),
            MarketStateLayoutV3::get_with_reqwest_with_program_id(api_url, &base_mint_pub, &quote_mint_pub, market_state_program_id));

        let liquidity_state = match liquidity_state {
            Ok(a) => a,
            Err(_) => {
                return Err(PoolError::GetLiquidityStateError)
            }
        };

        let market_state = match market_state {
            Ok(a) => a,
            Err(_) => {
                return Err(PoolError::GetMarketStateError)
            }
        };

        Ok(
            match Self::build(
                liquidity_state,
                market_state,
                cluster_type
            ) {
                Ok(a) => a,
                Err(_) => {
                    return Err(PoolError::BuildLiquidityInfoError)
                }
            }
        )
    }

    pub fn get_associated_id(amm_program_id: Pubkey, market_id: Pubkey) -> (Pubkey, u8) {
        Pubkey::find_program_address(
            &[
                &amm_program_id.to_bytes()[..],
                &market_id.to_bytes()[..],
                b"amm_associated_seed",
            ],
            &amm_program_id,
        )
    }

    pub fn get_associated_authority(amm_program_id: Pubkey) -> (Pubkey, u8) {
        Pubkey::find_program_address(
            &[
                b"amm authority"
            ],
            &amm_program_id,
        )
    }

    pub fn get_associated_base_vault(amm_program_id: Pubkey, market_id: Pubkey) -> (Pubkey, u8) {
        Pubkey::find_program_address(
            &[
                &amm_program_id.to_bytes()[..],
                &market_id.to_bytes()[..],
                b"coin_vault_associated_seed",
            ],
            &amm_program_id,
        )
    }

    pub fn get_associated_quote_vault(amm_program_id: Pubkey, market_id: Pubkey) -> (Pubkey, u8) {
        Pubkey::find_program_address(
            &[
                &amm_program_id.to_bytes()[..],
                &market_id.to_bytes()[..],
                b"pc_vault_associated_seed",
            ],
            &amm_program_id,
        )
    }
    pub fn get_associated_lp_mint(amm_program_id: Pubkey, market_id: Pubkey) -> (Pubkey, u8) {
        Pubkey::find_program_address(
            &[
                &amm_program_id.to_bytes()[..],
                &market_id.to_bytes()[..],
                b"lp_mint_associated_seed",
            ],
            &amm_program_id,
        )
    }

    pub fn get_associated_lp_vault(amm_program_id: Pubkey, market_id: Pubkey) -> (Pubkey, u8) {
        Pubkey::find_program_address(
            &[
                &amm_program_id.to_bytes()[..],
                &market_id.to_bytes()[..],
                b"temp_lp_token_associated_seed",
            ],
            &amm_program_id,
        )
    }

    pub fn get_associated_target_orders(amm_program_id: Pubkey, market_id: Pubkey) -> (Pubkey, u8) {
        Pubkey::find_program_address(
            &[
                &amm_program_id.to_bytes()[..],
                &market_id.to_bytes()[..],
                b"target_associated_seed",
            ],
            &amm_program_id,
        )
    }

    pub fn get_associated_withdraw_queue(amm_program_id: Pubkey, market_id: Pubkey) -> (Pubkey, u8) {
        Pubkey::find_program_address(
            &[
                &amm_program_id.to_bytes()[..],
                &market_id.to_bytes()[..],
                b"withdraw_associated_seed",
            ],
            &amm_program_id,
        )
    }
    pub fn get_associated_open_orders(amm_program_id: Pubkey, market_id: Pubkey) -> (Pubkey, u8) {
        Pubkey::find_program_address(
            &[
                &amm_program_id.to_bytes()[..],
                &market_id.to_bytes()[..],
                b"open_order_associated_seed",
            ],
            &amm_program_id,
        )
    }

    pub fn get_associated_config_id(amm_program_id: Pubkey) -> (Pubkey, u8) {
        Pubkey::find_program_address(
            &[
                b"amm_config_account_seed",
            ],
            &amm_program_id,
        )
    }

    pub fn get_market_authority(program_id: &Pubkey, market_id: &Pubkey) -> Option<Pubkey> {
        let mut nonce = 0;
        let mut public_key: Option<Pubkey> = None;

        while nonce < 100 {
            let seeds_with_nonce = &[market_id.as_ref(), &vec![nonce], &vec![0; 7]];
            let program_address = Pubkey::create_program_address(seeds_with_nonce, &program_id);
            match program_address {
                Ok(pub_key) => {
                    public_key = Some(pub_key);
                    break;
                }
                Err(_) => {}
            }
            nonce += 1;
        }

        public_key
    }
}