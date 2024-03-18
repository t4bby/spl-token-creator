use thiserror::Error;

#[derive(Debug, Error)]
#[allow(dead_code)]
pub enum DexScreenerError {
    #[error("Request error: {0}")]
    RequestError(String),

    #[error("Parse error: {0}")]
    ParseError(String),

    #[error("invalid pair")]
    InvalidPair,
}