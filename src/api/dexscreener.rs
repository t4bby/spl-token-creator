use reqwest::blocking::Response;
use crate::api::dexscreener::error::DexScreenerError;
use crate::api::dexscreener::types::{Pair, TokenResponse};

pub mod types;
pub mod error;
mod tests;


#[allow(dead_code)]
pub const DEXSCREENER_API: &str = "https://api.dexscreener.com/latest/dex";

#[allow(dead_code)]
pub struct DexScreener {
    client: reqwest::blocking::Client,
}

impl DexScreener {
    #[allow(dead_code)]
    pub fn new() -> Self {
        let client = reqwest::blocking::Client::new();
        Self {
            client,
        }
    }

    #[allow(dead_code)]
    pub fn parse_response(response: Response) -> Result<TokenResponse, DexScreenerError> {
        let res = response.json::<TokenResponse>();
        match res {
            Ok(a) => Ok(a),
            Err(e) => Err(DexScreenerError::ParseError(e.to_string()))
        }
    }

    #[allow(dead_code)]
    pub fn get_token_price(&self, token_address: &str) -> Result<f64, DexScreenerError> {
        let res = self.client.get(format!("{}/tokens/{}", DEXSCREENER_API, token_address))
                      .send()
                      .expect("failed to send getProgramAccounts request");

        let token_response = match Self::parse_response(res) {
            Ok(a) => a,
            Err(e) => {
                return Err(e);
            }
        };

        let pairs: Vec<Pair> = token_response.pairs.unwrap_or_else(|| {
            vec![]
        });

        if pairs.len() == 0 {
            return Err(DexScreenerError::InvalidPair);
        }

        Ok(pairs[0].price_native.parse::<f64>().unwrap())
    }
}
