use serde::Deserialize;

/// Global application settings loaded from environment variables.
#[derive(Debug, Clone, Deserialize)]
pub struct Settings {
    /// PostgreSQL connection URL.
    pub database_url: String,

    /// Tempo RPC endpoint URL.
    pub rpc_url: String,

    /// Block number to start indexing from (0 for genesis).
    pub start_block: u64,

    /// Number of blocks to fetch per batch.
    pub batch_size: u64,

    /// Port for the API server.
    pub api_port: u16,
}

impl Settings {
    /// Load settings from environment variables (with optional `.env` file).
    pub fn from_env() -> eyre::Result<Self> {
        dotenvy::dotenv().ok();

        Ok(Self {
            database_url: std::env::var("DATABASE_URL")
                .unwrap_or_else(|_| "postgres://tempulse:tempulse@localhost:5432/tempulse".into()),
            rpc_url: std::env::var("RPC_URL")
                .unwrap_or_else(|_| "https://rpc.moderato.tempo.xyz".into()),
            start_block: std::env::var("START_BLOCK")
                .unwrap_or_else(|_| "0".into())
                .parse()?,
            batch_size: std::env::var("BATCH_SIZE")
                .unwrap_or_else(|_| "100".into())
                .parse()?,
            api_port: std::env::var("API_PORT")
                .unwrap_or_else(|_| "3000".into())
                .parse()?,
        })
    }
}
