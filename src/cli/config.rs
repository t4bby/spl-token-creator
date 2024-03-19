use serde::{Deserialize, Serialize};

#[derive(Deserialize, Clone, Debug, Serialize)]
pub struct Config {
    pub rpc_url: String,
    pub wss_url: String,
    pub wallet_keypair: String,
    pub nft_storage_api_key: String,
    pub project_directory: String,
}

#[derive(Deserialize, Clone, Debug, Serialize)]
pub struct WalletFile {
    pub key: String,
}

#[derive(Deserialize, Serialize, Default, Clone)]
pub struct ProjectConfig {
    pub name: String,
    pub symbol: String,
    pub description: String,
    pub telegram: Option<String>,
    pub tags: Option<Vec<String>>,
    pub mint_amount: u64,
    pub decimal: u8,
    pub image_filename: String,
    pub metadata_uri: String,
    pub token_keypair: String,
    pub wallets: Vec<String>,
    pub wsol_wallets: Vec<String>,
}

#[derive(Deserialize, Serialize)]
pub struct MarketConfig {
    pub market_id: String,
    pub base_mint: String,
    pub quote_mint: String,
    pub market_keypair: String,
    pub bids_keypair: String,
    pub asks_keypair: String,
    pub request_keypair: String,
    pub event_keypair: String,
    pub base_vault_keypair: String,
    pub quote_vault_keypair: String,
    pub vault_signer_pk: String
}

#[derive(Deserialize, Serialize)]
pub struct LiquidityConfig {
    pub file_location: String,
    pub amm_id: String,
    pub amm_authority: String,
    pub amm_open_orders: String,
    pub lp_mint: String,
    pub coin_vault: String,
    pub pc_vault: String,
    pub target_orders: String,
    pub amm_config_id: String,
    pub base_token_account: String,
}