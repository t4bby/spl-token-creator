use reqwest::blocking::Response;
use crate::api::dexscreener::error::DexScreenerError;
use crate::api::dexscreener::types::{Pair, TokenResponse};
use crate::utils;

pub mod types;
mod error;


pub const DEXSCREENER_API: &str = "https://api.dexscreener.com/latest/dex";

#[test]
fn test_get_token_price() {
    let dex_screener = DexScreener::new();
    let k = dex_screener.get_token_price("tRPkMvRL1xm5hwLjM19FxsB5fdfJtLYTDr9W22RQAim");

    assert!(k.is_ok());

    let token_price = k.unwrap();
    println!("1 Token = {:.9} SOL Price", token_price);

    let token_to_sol = utils::math::token_price_to_sol(50f64, token_price);
    println!("50 Token = {:.9} SOL Price", token_to_sol);

    let sol_to_token = utils::math::sol_to_token_price(1f64, token_price);
    println!("1 SOL = {:.9} Token Price", sol_to_token);
}

#[test]
fn test_get_token_price_loop() {
    let dex_screener = DexScreener::new();

    let mut token_price = 0.0;
    let mut initial_price = 0.0;
    println!("Initial Price {}", initial_price);

    loop {
        let k = dex_screener.get_token_price("BaDLC4pEnwqtvBR9pYDRFJzYhTYGfHCia6GuUb6Bk9jg");
        match k {
            Ok(k) => {
                if token_price < k || token_price > k {
                    if initial_price ==  0f64.powi(9) {
                        println!("Took Initial Price");
                        initial_price = k;
                    }

                    token_price = k;
                    println!("Price changed!");
                    println!("1 Token = {:.9} SOL Price", token_price);
                    println!("Profit: {}", token_price - initial_price);
                    println!("Percent Increase: {}", ((token_price - initial_price) / initial_price) * 100f64);
                }
            }
            Err(e) => {
                println!("Error: {}", e);
            }
        }

        std::thread::sleep(std::time::Duration::from_millis(1000 / (300 / 60)));
    }
}



pub struct DexScreener {
    client: reqwest::blocking::Client,
}

impl DexScreener {
    pub fn new() -> Self {
        let client = reqwest::blocking::Client::new();
        Self {
            client,
        }
    }

    pub fn parse_response(response: Response) -> Result<TokenResponse, DexScreenerError> {
        let res = response.json::<TokenResponse>();
        match res {
            Ok(a) => Ok(a),
            Err(e) => Err(DexScreenerError::ParseError(e.to_string()))
        }
    }

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

        let pairs: Vec<Pair> = token_response.pairs.unwrap();

        Ok(pairs[0].price_native.parse::<f64>().unwrap())
    }
}
