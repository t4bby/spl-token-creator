#[allow(dead_code)]
pub fn token_price_to_sol(token_amount: f64, token_sol_price: f64) -> f64 {
    token_amount * token_sol_price
}

#[allow(dead_code)]
pub fn sol_to_token_price(sol_ammunt: f64, token_price: f64) -> f64 {
    sol_ammunt / token_price
}