use thiserror::Error;

#[derive(Debug, Error)]
#[allow(dead_code)]
pub enum PoolError {
    #[error("failed to get market authority")]
    GetMarketAuthorityError,
    #[error("failed to build liquidity info")]
    BuildLiquidityInfoError,
    #[error("failed to get market state")]
    GetMarketStateError,
    #[error("failed to get liquidity state")]
    GetLiquidityStateError,
}

#[derive(Debug, Error)]
#[allow(dead_code)]
pub enum RequestError {
    #[error("Error: {0}")]
    GetLiquidityStateRequestError(String),

    #[error("Error: {0}")]
    GetMarketStateRequestError(String),
}

#[derive(Debug, Error)]
#[allow(dead_code)]
pub enum ParserError {
    #[error("account not found")]
    AccountNotFound,

    #[error("account data not found")]
    AccountDataNotFound,

    #[error("account data decode error: {0}")]
    AccountDataDecodeError(String),
}
