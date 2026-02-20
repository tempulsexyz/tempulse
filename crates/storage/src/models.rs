use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

// ─── Token ──────────────────────────────────────────────────────────────────

/// A tracked TIP-20 stablecoin.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Token {
    pub address: String,
    pub name: String,
    pub symbol: String,
    pub decimals: i16,
    pub currency: String,
    pub total_supply: String,
    pub created_at_block: i64,
    pub created_at_tx: String,
}

// ─── Transfer ───────────────────────────────────────────────────────────────

/// An immutable record of a token movement (transfer, mint, or burn).
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Transfer {
    pub id: i64,
    pub token_address: String,
    pub from_address: String,
    pub to_address: String,
    pub amount: String,
    pub memo: Option<String>,
    pub event_type: String,
    pub transaction_hash: String,
    pub block_number: i64,
    pub log_index: i32,
    pub created_at: NaiveDateTime,
}

/// Insert-ready transfer (no `id` or `created_at`).
#[derive(Debug, Clone)]
pub struct NewTransfer {
    pub token_address: String,
    pub from_address: String,
    pub to_address: String,
    pub amount: String,
    pub memo: Option<String>,
    pub event_type: String,
    pub transaction_hash: String,
    pub block_number: i64,
    pub log_index: i32,
}

// ─── Account ────────────────────────────────────────────────────────────────

/// Current balance for an (address, token) pair.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Account {
    pub address: String,
    pub token_address: String,
    pub balance: String,
    pub updated_at_block: i64,
}

// ─── IndexedBlock ───────────────────────────────────────────────────────────

/// A block that has been processed by the indexer.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct IndexedBlock {
    pub block_number: i64,
    pub block_hash: String,
    pub parent_hash: String,
    pub timestamp: i64,
}

// ─── HourlyStats ────────────────────────────────────────────────────────────

/// Pre-aggregated hourly metrics for a token.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct HourlyStats {
    pub token_address: String,
    pub hour: NaiveDateTime,
    pub transfer_count: i64,
    pub transfer_volume: String,
    pub mint_count: i64,
    pub mint_volume: String,
    pub burn_count: i64,
    pub burn_volume: String,
    pub unique_senders: i64,
    pub unique_receivers: i64,
}
