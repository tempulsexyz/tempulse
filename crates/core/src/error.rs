use thiserror::Error;

/// Shared error type used across all Tempulse crates.
#[derive(Debug, Error)]
pub enum AppError {
    #[error("RPC error: {0}")]
    Rpc(String),

    #[error("Database error: {0}")]
    Database(String),

    #[error("Decode error: {0}")]
    Decode(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error(transparent)]
    Other(#[from] eyre::Error),
}
