//! Tempulse Indexer — crawls TIP-20 stablecoin events from the Tempo blockchain.
//!
//! Flow:
//! 1. Connect to Tempo RPC & PostgreSQL
//! 2. Discover existing tokens via TIP20Factory TokenCreated events
//! 3. Poll blocks in batches, decode Transfer/Mint/Burn events
//! 4. Persist to DB atomically and update account balances
//!
//! Production features:
//! - Reorg detection via parent hash comparison against indexed_blocks
//! - Atomic writes per batch (transfers + balances + blocks + cursor in one transaction)
//! - total_supply tracked on mint/burn
//! - hourly_stats aggregated in real-time

use alloy::{
    consensus::BlockHeader,
    network::primitives::HeaderResponse,
    primitives::{Address, address},
    providers::Provider,
    rpc::types::Filter,
    sol_types::SolEvent,
};
use chrono::{DateTime, Timelike};
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
/// ## Production Features
///
/// 1. **Reorg detection** — checks parent hash of the first block in the batch against
///    `indexed_blocks`. If mismatch, walks backward to find the fork point, rolls back,
///    and re-indexes from there.
/// 2. **Atomic writes** — all transfers, balance updates, block records, hourly stats,
///    and the cursor update happen inside a single database transaction.
/// 3. **total_supply tracking** — mint/burn events increment/decrement the token's supply.
/// 4. **hourly_stats aggregation** — real-time aggregation into the hourly_stats table.
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

    // ── Reorg Detection ────────────────────────────────────────────────
    // Check if the parent hash of block `from` matches what we stored for block `from - 1`.
    if from > 1 {
        if let Some(stored_hash) = storage::repos::get_block_hash(pool, from - 1).await? {
            // Fetch the actual block from the chain to compare parent hashes
            let block = provider
                .get_block_by_number(alloy::eips::BlockNumberOrTag::Number(from as u64))
                .await?
                .ok_or_else(|| eyre::eyre!("Block {} not found on chain", from))?;
            let parent_hash = format!("{:#x}", block.header.parent_hash());
            if parent_hash != stored_hash {
                tracing::warn!(
                    block = from,
                    expected = %stored_hash,
                    got = %parent_hash,
                    "Reorg detected! Rolling back…"
                );

                // Walk backward to find the fork point
                let mut fork_block = from - 2;
                while fork_block > 0 {
                    if let Some(stored) = storage::repos::get_block_hash(pool, fork_block).await? {
                        let chain_block = provider
                            .get_block_by_number(alloy::eips::BlockNumberOrTag::Number(
                                fork_block as u64,
                            ))
                            .await?
                            .ok_or_else(|| {
                                eyre::eyre!(
                                    "Block {} not found on chain during reorg detection",
                                    fork_block
                                )
                            })?;
                        let chain_hash = format!("{:#x}", chain_block.header.hash());
                        if chain_hash == stored {
                            break; // Found the fork point
                        }
                    } else {
                        break; // No stored hash, can't go further back
                    }
                    fork_block -= 1;
                }

                tracing::warn!(
                    fork_block = fork_block,
                    "Fork point found, rolling back to block"
                );
                storage::repos::reorg_rollback(pool, fork_block).await?;
                *last_block = fork_block;
                return Ok(true); // Signal that work was done (rollback), re-index next iteration
            }
        }
    }

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
    let transfer_filter = Filter::new()
        .event_signature(TIP20::Transfer::SIGNATURE_HASH)
        .from_block(from as u64)
        .to_block(to as u64);

    let transfer_logs = provider.get_logs(&transfer_filter).await?;
    tracing::info!(count = transfer_logs.len(), "Fetched transfer logs");

    let mut new_transfers: Vec<NewTransfer> = Vec::new();

    // Collect balance updates and stats to apply inside the transaction
    struct BalanceUpdate {
        address: String,
        token_address: String,
        amount: String,
        is_add: bool,
        block_number: i64,
    }

    struct StatsUpdate {
        token_address: String,
        event_type: String,
        amount: String,
        sender: String,
        receiver: String,
        timestamp: i64,
    }

    let mut balance_updates: Vec<BalanceUpdate> = Vec::new();
    let mut stats_updates: Vec<StatsUpdate> = Vec::new();
    let mut supply_updates: Vec<(String, String, bool)> = Vec::new(); // (token, amount, is_mint)

    for log in &transfer_logs {
        let log_address = log.address();

        // ── Prefix check: skip non-TIP-20 contracts ─────────────────
        if !is_tip20_address(&log_address) {
            continue;
        }

        // ── Ensure this token is registered in the DB ───────────────
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

            // Collect balance updates
            if event_type == "transfer" || event_type == "mint" {
                balance_updates.push(BalanceUpdate {
                    address: to_addr.clone(),
                    token_address: token_addr_str.clone(),
                    amount: amount_str.clone(),
                    is_add: true,
                    block_number: block_num,
                });
            }
            if event_type == "transfer" || event_type == "burn" {
                balance_updates.push(BalanceUpdate {
                    address: from_addr.clone(),
                    token_address: token_addr_str.clone(),
                    amount: amount_str.clone(),
                    is_add: false,
                    block_number: block_num,
                });
            }

            // Collect supply updates for mint/burn
            if event_type == "mint" {
                supply_updates.push((token_addr_str.clone(), amount_str.clone(), true));
            } else if event_type == "burn" {
                supply_updates.push((token_addr_str.clone(), amount_str.clone(), false));
            }

            // Collect hourly stats updates
            // Use block_number as a proxy timestamp — we'll derive the hour from `created_at`
            // which defaults to NOW() in the DB. For accuracy, use the log's block timestamp.
            let block_timestamp = log.block_timestamp.unwrap_or(0);
            stats_updates.push(StatsUpdate {
                token_address: token_addr_str.clone(),
                event_type: event_type.to_string(),
                amount: amount_str.clone(),
                sender: from_addr.clone(),
                receiver: to_addr.clone(),
                timestamp: block_timestamp as i64,
            });
        }
    }

    // ── Atomic write: wrap everything in a transaction ──────────────
    let mut tx = pool.begin().await?;

    // 1. Persist transfers (true batch insert)
    if !new_transfers.is_empty() {
        tracing::info!(count = new_transfers.len(), "Persisting transfers");
        storage::repos::insert_transfers_batch(&mut *tx, &new_transfers).await?;
    }

    // 2. Apply balance updates
    for bu in &balance_updates {
        storage::repos::upsert_account_balance(
            &mut *tx,
            &bu.address,
            &bu.token_address,
            &bu.amount,
            bu.is_add,
            bu.block_number,
        )
        .await?;
    }

    // 3. Apply total_supply updates
    for (token_addr, amount, is_mint) in &supply_updates {
        storage::repos::update_total_supply_on_event(&mut *tx, token_addr, amount, *is_mint)
            .await?;
    }

    // 4. Apply hourly stats
    for su in &stats_updates {
        let hour = if su.timestamp > 0 {
            DateTime::from_timestamp(su.timestamp, 0)
                .unwrap_or_default()
                .naive_utc()
                .date()
                .and_hms_opt(
                    DateTime::from_timestamp(su.timestamp, 0)
                        .unwrap_or_default()
                        .naive_utc()
                        .time()
                        .hour() as u32,
                    0,
                    0,
                )
                .unwrap_or_default()
        } else {
            // Fallback: use current time truncated to hour
            chrono::Utc::now()
                .naive_utc()
                .date()
                .and_hms_opt(chrono::Utc::now().naive_utc().time().hour() as u32, 0, 0)
                .unwrap_or_default()
        };

        storage::repos::upsert_hourly_stats(
            &mut *tx,
            &su.token_address,
            hour,
            &su.event_type,
            &su.amount,
            &su.sender,
            &su.receiver,
        )
        .await?;
    }

    // 5. Record indexed blocks for this batch (for reorg detection)
    // We record the last block in the batch at minimum
    if let Some(block) = provider
        .get_block_by_number(alloy::eips::BlockNumberOrTag::Number(to as u64))
        .await?
    {
        let indexed_block = IndexedBlock {
            block_number: to,
            block_hash: format!("{:#x}", block.header.hash()),
            parent_hash: format!("{:#x}", block.header.parent_hash()),
            timestamp: block.header.timestamp() as i64,
        };
        storage::repos::insert_block(&mut *tx, &indexed_block).await?;
    }

    // 6. Update cursor
    storage::repos::set_last_indexed_block(&mut *tx, to).await?;

    // ── Commit the transaction ──────────────────────────────────────
    tx.commit().await?;

    *last_block = to;

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
