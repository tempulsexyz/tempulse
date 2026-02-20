//! Tempulse Indexer — crawls TIP-20 stablecoin events from the Tempo blockchain.
//!
//! Flow:
//! 1. Connect to Tempo RPC & PostgreSQL
//! 2. Discover existing tokens via TIP20Factory TokenCreated events
//! 3. Poll blocks in batches, decode Transfer/Mint/Burn events
//! 4. Persist to DB and update account balances
//!
//! Optimization: Instead of loading all tracked token addresses into memory
//! (which fails at scale), we use the TIP-20 address prefix to identify
//! valid tokens at O(1) cost per log, then lazily register unknown tokens via DB.

use alloy::{
    primitives::{Address, address},
    providers::Provider,
    rpc::types::Filter,
    sol_types::SolEvent,
};
use eyre::Result;
use tempulse_core::{Settings, telemetry};
use tempulse_storage::{self as storage, models::*};
use tempulse_tempo::{TIP20, TIP20Factory, decoder, provider};

/// TIP20Factory precompile address on Tempo.
const FACTORY_ADDRESS: Address = address!("20Fc000000000000000000000000000000000000");

/// Address zero — used in mint/burn detection.
const ZERO_ADDRESS: Address = Address::ZERO;

/// The 12-byte prefix shared by ALL TIP-20 token addresses.
/// Any address starting with this prefix is a TIP-20 token.
const TIP20_PREFIX: [u8; 12] = [
    0x20, 0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
];

/// Check if an address is a valid TIP-20 token by its prefix.
/// This is an O(1) check that requires zero memory — no need to load token lists.
#[inline]
fn is_tip20_address(addr: &Address) -> bool {
    addr.as_slice()[..12] == TIP20_PREFIX
}

#[tokio::main]
async fn main() -> Result<()> {
    // ── Initialisation ──────────────────────────────────────────────────
    telemetry::init();
    let settings = Settings::from_env()?;

    tracing::info!(rpc = %settings.rpc_url, "Starting Tempulse Indexer");

    // Connect to the database
    let pool = storage::connect(&settings.database_url).await?;
    tracing::info!("Connected to database");

    // Run migrations
    sqlx::migrate!("../../migrations").run(&pool).await?;
    tracing::info!("Database migrations applied");

    // Create Tempo RPC provider
    let provider = provider::create_provider(&settings.rpc_url)?;
    tracing::info!("Connected to Tempo RPC");

    // ── Token Discovery ─────────────────────────────────────────────────
    // Fetch all TokenCreated events from the Factory to seed the token registry.
    // This only scans Factory events (one contract), so it's lightweight.
    tracing::info!("Discovering TIP-20 tokens from Factory…");
    discover_tokens(&provider, &pool, &settings).await?;

    let token_count = storage::repos::get_token_count(&pool).await?;
    tracing::info!(count = token_count, "Tracking tokens");

    // ── Main Indexing Loop ──────────────────────────────────────────────
    let mut last_block = storage::repos::get_last_indexed_block(&pool).await?;
    if last_block == 0 && settings.start_block > 0 {
        last_block = settings.start_block as i64 - 1;
    }

    tracing::info!(from_block = last_block + 1, "Starting indexing loop");

    let shutdown = tokio::signal::ctrl_c();
    tokio::pin!(shutdown);

    loop {
        // Check for shutdown
        tokio::select! {
            _ = &mut shutdown => {
                tracing::info!("Shutting down gracefully…");
                break;
            }
            result = index_next_batch(&provider, &pool, &mut last_block, &settings) => {
                match result {
                    Ok(indexed) => {
                        if !indexed {
                            // We're caught up — wait before polling again
                            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                        }
                    }
                    Err(e) => {
                        tracing::error!(error = %e, "Indexing error, retrying in 5s…");
                        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                    }
                }
            }
        }
    }

    tracing::info!("Indexer stopped.");
    Ok(())
}

/// Discover tokens by querying TIP20Factory TokenCreated events.
///
/// This is safe at scale because it only queries ONE contract address
/// (the Factory), not all tokens.
async fn discover_tokens(
    provider: &tempulse_tempo::provider::TempoProvider,
    pool: &sqlx::PgPool,
    settings: &Settings,
) -> Result<()> {
    let chain_head = provider.get_block_number().await?;

    // Scan from genesis (or start_block) to chain head for factory events
    let mut from = settings.start_block;
    let batch = 10_000u64;

    while from <= chain_head {
        let to = std::cmp::min(from + batch - 1, chain_head);

        let filter = Filter::new()
            .address(FACTORY_ADDRESS)
            .event_signature(TIP20Factory::TokenCreated::SIGNATURE_HASH)
            .from_block(from)
            .to_block(to);

        let logs = provider.get_logs(&filter).await?;

        for log in &logs {
            if let Some(event) = decoder::decode_factory_log(log) {
                tracing::info!(
                    token = %event.token_address,
                    name = %event.name,
                    symbol = %event.symbol,
                    currency = %event.currency,
                    "Discovered token"
                );

                let token = Token {
                    address: format!("{:#x}", event.token_address),
                    name: event.name,
                    symbol: event.symbol,
                    decimals: 6, // TIP-20 tokens always have 6 decimals
                    currency: event.currency,
                    total_supply: "0".to_string(),
                    created_at_block: event.block_number as i64,
                    created_at_tx: event.transaction_hash,
                };

                storage::repos::insert_token(pool, &token).await?;
            }
        }

        from = to + 1;
    }

    Ok(())
}

/// Index the next batch of blocks. Returns `Ok(true)` if work was done, `Ok(false)` if caught up.
///
/// ## Scalability Strategy
///
/// Instead of loading all token addresses into memory and passing them to the RPC filter
/// (which breaks with millions of tokens), this function:
///
/// 1. **Filters by event signature only** — queries Transfer events across ALL addresses.
/// 2. **Validates via TIP-20 prefix** — checks `is_tip20_address()` on each log (O(1), zero memory).
/// 3. **Lazily registers unknown tokens** — if a log has the TIP-20 prefix but the token isn't
///    in the DB yet, it inserts a placeholder and continues. This handles tokens created outside
///    the Factory or discovered mid-batch.
async fn index_next_batch(
    provider: &tempulse_tempo::provider::TempoProvider,
    pool: &sqlx::PgPool,
    last_block: &mut i64,
    settings: &Settings,
) -> Result<bool> {
    let chain_head = provider.get_block_number().await?;
    let chain_head = chain_head as i64;

    if *last_block >= chain_head {
        return Ok(false); // Caught up
    }

    let from = *last_block + 1;
    let to = std::cmp::min(from + settings.batch_size as i64 - 1, chain_head);

    tracing::info!(from = from, to = to, head = chain_head, "Indexing batch");

    // ── Fetch Factory events (new tokens in this batch) ─────────────
    let factory_filter = Filter::new()
        .address(FACTORY_ADDRESS)
        .event_signature(TIP20Factory::TokenCreated::SIGNATURE_HASH)
        .from_block(from as u64)
        .to_block(to as u64);

    let factory_logs = provider.get_logs(&factory_filter).await?;

    for log in &factory_logs {
        if let Some(event) = decoder::decode_factory_log(log) {
            tracing::info!(
                token = %event.token_address,
                symbol = %event.name,
                "New token discovered mid-indexing"
            );
            let token = Token {
                address: format!("{:#x}", event.token_address),
                name: event.name,
                symbol: event.symbol,
                decimals: 6,
                currency: event.currency,
                total_supply: "0".to_string(),
                created_at_block: event.block_number as i64,
                created_at_tx: event.transaction_hash,
            };
            storage::repos::insert_token(pool, &token).await?;
        }
    }

    // ── Fetch Transfer events — filter by event signature only ──────
    // No address filter needed. We validate each log's address via the
    // TIP-20 prefix check, which is O(1) and uses zero extra memory.
    let transfer_filter = Filter::new()
        .event_signature(TIP20::Transfer::SIGNATURE_HASH)
        .from_block(from as u64)
        .to_block(to as u64);

    let transfer_logs = provider.get_logs(&transfer_filter).await?;
    tracing::info!(count = transfer_logs.len(), "Fetched transfer logs");

    let mut new_transfers: Vec<NewTransfer> = Vec::new();

    for log in &transfer_logs {
        let log_address = log.address();

        // ── Prefix check: skip non-TIP-20 contracts ─────────────────
        if !is_tip20_address(&log_address) {
            continue;
        }

        // ── Ensure this token is registered in the DB ───────────────
        // Uses ON CONFLICT DO NOTHING, so this is safe to call repeatedly.
        let token_addr_str = format!("{:#x}", log_address);
        ensure_token_registered(pool, &token_addr_str, from).await?;

        if let Some(event) = decoder::decode_tip20_log(log) {
            let (from_addr, to_addr, amount_str, event_type, memo, block_num, tx_hash, idx) =
                match &event {
                    decoder::Tip20Event::Transfer {
                        from,
                        to,
                        amount,
                        block_number,
                        transaction_hash,
                        log_index,
                        ..
                    } => (
                        format!("{:#x}", from),
                        format!("{:#x}", to),
                        amount.to_string(),
                        "transfer",
                        None,
                        *block_number as i64,
                        transaction_hash.clone(),
                        *log_index as i32,
                    ),
                    decoder::Tip20Event::Mint {
                        to,
                        amount,
                        block_number,
                        transaction_hash,
                        log_index,
                        ..
                    } => (
                        format!("{:#x}", ZERO_ADDRESS),
                        format!("{:#x}", to),
                        amount.to_string(),
                        "mint",
                        None,
                        *block_number as i64,
                        transaction_hash.clone(),
                        *log_index as i32,
                    ),
                    decoder::Tip20Event::Burn {
                        from,
                        amount,
                        block_number,
                        transaction_hash,
                        log_index,
                        ..
                    } => (
                        format!("{:#x}", from),
                        format!("{:#x}", ZERO_ADDRESS),
                        amount.to_string(),
                        "burn",
                        None,
                        *block_number as i64,
                        transaction_hash.clone(),
                        *log_index as i32,
                    ),
                    decoder::Tip20Event::TransferWithMemo {
                        from,
                        to,
                        amount,
                        memo,
                        block_number,
                        transaction_hash,
                        log_index,
                        ..
                    } => (
                        format!("{:#x}", from),
                        format!("{:#x}", to),
                        amount.to_string(),
                        "transfer",
                        Some(format!("0x{}", hex::encode(memo))),
                        *block_number as i64,
                        transaction_hash.clone(),
                        *log_index as i32,
                    ),
                };

            new_transfers.push(NewTransfer {
                token_address: token_addr_str.clone(),
                from_address: from_addr.clone(),
                to_address: to_addr.clone(),
                amount: amount_str.clone(),
                memo,
                event_type: event_type.to_string(),
                transaction_hash: tx_hash.clone(),
                block_number: block_num,
                log_index: idx,
            });

            // Update balances
            if event_type == "transfer" || event_type == "mint" {
                storage::repos::upsert_account_balance(
                    pool,
                    &to_addr,
                    &token_addr_str,
                    &amount_str,
                    true,
                    block_num,
                )
                .await?;
            }
            if event_type == "transfer" || event_type == "burn" {
                storage::repos::upsert_account_balance(
                    pool,
                    &from_addr,
                    &token_addr_str,
                    &amount_str,
                    false,
                    block_num,
                )
                .await?;
            }
        }
    }

    // Persist transfers
    if !new_transfers.is_empty() {
        tracing::info!(count = new_transfers.len(), "Persisting transfers");
        storage::repos::insert_transfers_batch(pool, &new_transfers).await?;
    }

    // Update cursor
    *last_block = to;
    storage::repos::set_last_indexed_block(pool, to).await?;

    tracing::info!(
        block = to,
        transfers = new_transfers.len(),
        "Batch complete"
    );

    Ok(true)
}

/// Ensure a TIP-20 token address is registered in the DB.
///
/// If the token was discovered via the Factory, it will already exist.
/// Otherwise, insert a placeholder with minimal metadata (it can be enriched later).
/// Uses ON CONFLICT DO NOTHING so it's safe to call on every log.
async fn ensure_token_registered(
    pool: &sqlx::PgPool,
    token_address: &str,
    block_number: i64,
) -> Result<()> {
    let token = Token {
        address: token_address.to_string(),
        name: String::new(),
        symbol: String::new(),
        decimals: 6,
        currency: String::new(),
        total_supply: "0".to_string(),
        created_at_block: block_number,
        created_at_tx: String::new(),
    };
    storage::repos::insert_token(pool, &token).await?;
    Ok(())
}
