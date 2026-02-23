use chrono::NaiveDateTime;
use sqlx::{Executor, PgPool, Postgres, QueryBuilder};

use crate::models::*;

// ─── Token Queries ──────────────────────────────────────────────────────────

/// Insert a new token (ignore if already exists).
pub async fn insert_token(pool: &PgPool, token: &Token) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO tokens (address, name, symbol, decimals, currency, total_supply, created_at_block, created_at_tx)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        ON CONFLICT (address) DO NOTHING
        "#,
    )
    .bind(&token.address)
    .bind(&token.name)
    .bind(&token.symbol)
    .bind(token.decimals)
    .bind(&token.currency)
    .bind(&token.total_supply)
    .bind(token.created_at_block)
    .bind(&token.created_at_tx)
    .execute(pool)
    .await?;
    Ok(())
}

/// Get all tracked tokens.
pub async fn get_all_tokens(pool: &PgPool) -> Result<Vec<Token>, sqlx::Error> {
    sqlx::query_as::<_, Token>("SELECT * FROM tokens ORDER BY symbol")
        .fetch_all(pool)
        .await
}

/// Get a single token by address.
pub async fn get_token(pool: &PgPool, address: &str) -> Result<Option<Token>, sqlx::Error> {
    sqlx::query_as::<_, Token>("SELECT * FROM tokens WHERE address = $1")
        .bind(address)
        .fetch_optional(pool)
        .await
}

/// Get the count of tracked tokens (no allocation — just a scalar).
pub async fn get_token_count(pool: &PgPool) -> Result<i64, sqlx::Error> {
    let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM tokens")
        .fetch_one(pool)
        .await?;
    Ok(row.0)
}

/// Update total supply for a token.
pub async fn update_total_supply(
    pool: &PgPool,
    token_address: &str,
    new_supply: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE tokens SET total_supply = $1 WHERE address = $2")
        .bind(new_supply)
        .bind(token_address)
        .execute(pool)
        .await?;
    Ok(())
}

// ─── Transfer Queries ───────────────────────────────────────────────────────

/// Insert a batch of transfers using a single multi-value INSERT.
/// Skips duplicates on (transaction_hash, log_index).
pub async fn insert_transfers_batch<'e, E>(
    executor: E,
    transfers: &[NewTransfer],
) -> Result<(), sqlx::Error>
where
    E: Executor<'e, Database = Postgres>,
{
    if transfers.is_empty() {
        return Ok(());
    }

    // Build: INSERT INTO transfers (...) VALUES (...), (...), ... ON CONFLICT DO NOTHING
    let mut qb: QueryBuilder<Postgres> = QueryBuilder::new(
        "INSERT INTO transfers (token_address, from_address, to_address, amount, memo, event_type, transaction_hash, block_number, log_index) ",
    );

    qb.push_values(transfers, |mut b, t| {
        b.push_bind(&t.token_address)
            .push_bind(&t.from_address)
            .push_bind(&t.to_address)
            .push_bind(&t.amount)
            .push_bind(&t.memo)
            .push_bind(&t.event_type)
            .push_bind(&t.transaction_hash)
            .push_bind(t.block_number)
            .push_bind(t.log_index);
    });

    qb.push(" ON CONFLICT (transaction_hash, log_index) DO NOTHING");
    qb.build().execute(executor).await?;
    Ok(())
}

/// Get the most recent transfers across all tokens.
pub async fn get_recent_transfers(pool: &PgPool, limit: i64) -> Result<Vec<Transfer>, sqlx::Error> {
    sqlx::query_as::<_, Transfer>(
        "SELECT * FROM transfers ORDER BY block_number DESC, log_index DESC LIMIT $1",
    )
    .bind(limit)
    .fetch_all(pool)
    .await
}

/// Get transfers for a specific token.
pub async fn get_token_transfers(
    pool: &PgPool,
    token_address: &str,
    limit: i64,
) -> Result<Vec<Transfer>, sqlx::Error> {
    sqlx::query_as::<_, Transfer>(
        "SELECT * FROM transfers WHERE token_address = $1 ORDER BY block_number DESC, log_index DESC LIMIT $2",
    )
    .bind(token_address)
    .bind(limit)
    .fetch_all(pool)
    .await
}

// ─── Total Supply ───────────────────────────────────────────────────────────

/// Update a token's total_supply in response to mint or burn events.
/// For mints: `is_mint = true` → supply increases.
/// For burns: `is_mint = false` → supply decreases (clamped at 0).
pub async fn update_total_supply_on_event<'e, E>(
    executor: E,
    token_address: &str,
    amount: &str,
    is_mint: bool,
) -> Result<(), sqlx::Error>
where
    E: Executor<'e, Database = Postgres>,
{
    if is_mint {
        sqlx::query(
            r#"
            UPDATE tokens
            SET total_supply = (CAST(total_supply AS NUMERIC) + CAST($1 AS NUMERIC))::TEXT
            WHERE address = $2
            "#,
        )
        .bind(amount)
        .bind(token_address)
        .execute(executor)
        .await?;
    } else {
        sqlx::query(
            r#"
            UPDATE tokens
            SET total_supply = GREATEST(0, CAST(total_supply AS NUMERIC) - CAST($1 AS NUMERIC))::TEXT
            WHERE address = $2
            "#,
        )
        .bind(amount)
        .bind(token_address)
        .execute(executor)
        .await?;
    }
    Ok(())
}

// ─── Hourly Stats ───────────────────────────────────────────────────────────

/// Upsert a row into hourly_stats, incrementing counters and volumes.
pub async fn upsert_hourly_stats<'e, E>(
    executor: E,
    token_address: &str,
    hour: NaiveDateTime,
    event_type: &str,
    amount: &str,
    sender: &str,
    receiver: &str,
) -> Result<(), sqlx::Error>
where
    E: Executor<'e, Database = Postgres>,
{
    let (tc, tv, mc, mv, bc, bv) = match event_type {
        "mint" => (0i64, "0", 1i64, amount, 0i64, "0"),
        "burn" => (0, "0", 0, "0", 1, amount),
        _ => (1, amount, 0, "0", 0, "0"),
    };

    // We use a simple approach: track unique senders/receivers is expensive
    // in a single upsert, so we increment by 1 for non-zero addresses.
    let is_sender = if sender != "0x0000000000000000000000000000000000000000" {
        1i64
    } else {
        0
    };
    let is_receiver = if receiver != "0x0000000000000000000000000000000000000000" {
        1i64
    } else {
        0
    };

    sqlx::query(
        r#"
        INSERT INTO hourly_stats (token_address, hour, transfer_count, transfer_volume, mint_count, mint_volume, burn_count, burn_volume, unique_senders, unique_receivers)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
        ON CONFLICT (token_address, hour) DO UPDATE SET
            transfer_count = hourly_stats.transfer_count + $3,
            transfer_volume = (CAST(hourly_stats.transfer_volume AS NUMERIC) + CAST($4 AS NUMERIC))::TEXT,
            mint_count = hourly_stats.mint_count + $5,
            mint_volume = (CAST(hourly_stats.mint_volume AS NUMERIC) + CAST($6 AS NUMERIC))::TEXT,
            burn_count = hourly_stats.burn_count + $7,
            burn_volume = (CAST(hourly_stats.burn_volume AS NUMERIC) + CAST($8 AS NUMERIC))::TEXT,
            unique_senders = hourly_stats.unique_senders + $9,
            unique_receivers = hourly_stats.unique_receivers + $10
        "#,
    )
    .bind(token_address)
    .bind(hour)
    .bind(tc)
    .bind(tv)
    .bind(mc)
    .bind(mv)
    .bind(bc)
    .bind(bv)
    .bind(is_sender)
    .bind(is_receiver)
    .execute(executor)
    .await?;
    Ok(())
}

// ─── Account Queries ────────────────────────────────────────────────────────

/// Upsert an account balance by adding a delta.
/// For mints: address = recipient, delta > 0.
/// For burns: address = sender, delta is subtracted.
/// For transfers: called twice — subtract from sender, add to receiver.
pub async fn upsert_account_balance<'e, E>(
    executor: E,
    address: &str,
    token_address: &str,
    amount: &str,
    is_add: bool,
    block_number: i64,
) -> Result<(), sqlx::Error>
where
    E: Executor<'e, Database = Postgres>,
{
    // Using text arithmetic via CAST.
    // This is safe because amounts are always positive unsigned integers as strings.
    if is_add {
        sqlx::query(
            r#"
            INSERT INTO accounts (address, token_address, balance, updated_at_block)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (address, token_address) DO UPDATE
            SET balance = (CAST(accounts.balance AS NUMERIC) + CAST($3 AS NUMERIC))::TEXT,
                updated_at_block = $4
            "#,
        )
        .bind(address)
        .bind(token_address)
        .bind(amount)
        .bind(block_number)
        .execute(executor)
        .await?;
    } else {
        sqlx::query(
            r#"
            INSERT INTO accounts (address, token_address, balance, updated_at_block)
            VALUES ($1, $2, '0', $4)
            ON CONFLICT (address, token_address) DO UPDATE
            SET balance = GREATEST(0, CAST(accounts.balance AS NUMERIC) - CAST($3 AS NUMERIC))::TEXT,
                updated_at_block = $4
            "#,
        )
        .bind(address)
        .bind(token_address)
        .bind(amount)
        .bind(block_number)
        .execute(executor)
        .await?;
    }
    Ok(())
}

/// Get top holders for a token, ordered by balance descending.
pub async fn get_top_holders(
    pool: &PgPool,
    token_address: &str,
    limit: i64,
) -> Result<Vec<Account>, sqlx::Error> {
    sqlx::query_as::<_, Account>(
        r#"
        SELECT * FROM accounts
        WHERE token_address = $1 AND CAST(balance AS NUMERIC) > 0
        ORDER BY CAST(balance AS NUMERIC) DESC
        LIMIT $2
        "#,
    )
    .bind(token_address)
    .bind(limit)
    .fetch_all(pool)
    .await
}

// ─── Block Queries ──────────────────────────────────────────────────────────

/// Insert a processed block.
pub async fn insert_block<'e, E>(executor: E, block: &IndexedBlock) -> Result<(), sqlx::Error>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query(
        r#"
        INSERT INTO indexed_blocks (block_number, block_hash, parent_hash, timestamp)
        VALUES ($1, $2, $3, $4)
        ON CONFLICT (block_number) DO UPDATE
        SET block_hash = $2, parent_hash = $3, timestamp = $4
        "#,
    )
    .bind(block.block_number)
    .bind(&block.block_hash)
    .bind(&block.parent_hash)
    .bind(block.timestamp)
    .execute(executor)
    .await?;
    Ok(())
}

/// Get the latest indexed block number.
pub async fn get_latest_block(pool: &PgPool) -> Result<Option<i64>, sqlx::Error> {
    let row: Option<(i64,)> = sqlx::query_as("SELECT MAX(block_number) FROM indexed_blocks")
        .fetch_optional(pool)
        .await?;
    Ok(row.and_then(|r| Some(r.0)))
}

/// Delete all indexed data after a given block number (for reorg handling).
/// This runs inside a single transaction to ensure consistency.
pub async fn reorg_rollback(pool: &PgPool, fork_block: i64) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;

    // Delete hourly_stats entries that may have been affected
    // (conservative: delete stats for hours containing blocks after fork)
    sqlx::query(
        r#"
        DELETE FROM hourly_stats
        WHERE (token_address, hour) IN (
            SELECT DISTINCT token_address, DATE_TRUNC('hour', created_at) AS hour
            FROM transfers WHERE block_number > $1
        )
        "#,
    )
    .bind(fork_block)
    .execute(&mut *tx)
    .await?;

    // Delete transfers after the fork point
    sqlx::query("DELETE FROM transfers WHERE block_number > $1")
        .bind(fork_block)
        .execute(&mut *tx)
        .await?;

    // Delete indexed blocks after the fork point
    sqlx::query("DELETE FROM indexed_blocks WHERE block_number > $1")
        .bind(fork_block)
        .execute(&mut *tx)
        .await?;

    // Reset the indexer cursor to the fork point
    sqlx::query("UPDATE indexer_state SET value = $1 WHERE key = 'last_indexed_block'")
        .bind(fork_block.to_string())
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;

    tracing::warn!(
        fork_block = fork_block,
        "Reorg rollback complete — deleted data after block"
    );
    Ok(())
}

/// Get the stored block hash for a specific block number.
pub async fn get_block_hash(
    pool: &PgPool,
    block_number: i64,
) -> Result<Option<String>, sqlx::Error> {
    let row: Option<(String,)> =
        sqlx::query_as("SELECT block_hash FROM indexed_blocks WHERE block_number = $1")
            .bind(block_number)
            .fetch_optional(pool)
            .await?;
    Ok(row.map(|r| r.0))
}

// ─── Indexer State ──────────────────────────────────────────────────────────

/// Get the last indexed block from persistent state.
pub async fn get_last_indexed_block(pool: &PgPool) -> Result<i64, sqlx::Error> {
    let row: (String,) =
        sqlx::query_as("SELECT value FROM indexer_state WHERE key = 'last_indexed_block'")
            .fetch_one(pool)
            .await?;
    Ok(row.0.parse::<i64>().unwrap_or(0))
}

/// Set the last indexed block in persistent state.
pub async fn set_last_indexed_block<'e, E>(
    executor: E,
    block_number: i64,
) -> Result<(), sqlx::Error>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query("UPDATE indexer_state SET value = $1 WHERE key = 'last_indexed_block'")
        .bind(block_number.to_string())
        .execute(executor)
        .await?;
    Ok(())
}

// ─── Analytics Queries ──────────────────────────────────────────────────────

/// Per-token transfer volume — total value transferred per token.
/// Returns (address, symbol, total_volume, transfer_count).
pub async fn get_token_volumes(
    pool: &PgPool,
) -> Result<Vec<(String, String, String, i64)>, sqlx::Error> {
    let rows: Vec<(String, String, String, i64)> = sqlx::query_as(
        r#"
        SELECT t.address, t.symbol,
               COALESCE(SUM(CAST(tr.amount AS NUMERIC)), 0)::TEXT AS total_volume,
               COUNT(tr.id) AS transfer_count
        FROM tokens t
        LEFT JOIN transfers tr ON t.address = tr.token_address
        GROUP BY t.address, t.symbol
        ORDER BY COALESCE(SUM(CAST(tr.amount AS NUMERIC)), 0) DESC
        "#,
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Global aggregate stats — total value transferred and total transactions across all tokens.
/// Returns (total_volume, total_transfers).
pub async fn get_global_stats(pool: &PgPool) -> Result<(String, i64), sqlx::Error> {
    let row: (String, i64) = sqlx::query_as(
        r#"
        SELECT COALESCE(SUM(CAST(amount AS NUMERIC)), 0)::TEXT,
               COUNT(*)
        FROM transfers
        "#,
    )
    .fetch_one(pool)
    .await?;
    Ok(row)
}

/// Total unique active addresses (senders + receivers).
pub async fn get_active_address_count(pool: &PgPool) -> Result<i64, sqlx::Error> {
    let row: (i64,) = sqlx::query_as(
        r#"
        SELECT COUNT(DISTINCT addr) FROM (
            SELECT from_address AS addr FROM transfers WHERE from_address != '0x0000000000000000000000000000000000000000'
            UNION
            SELECT to_address AS addr FROM transfers WHERE to_address != '0x0000000000000000000000000000000000000000'
        ) sub
        "#,
    )
    .fetch_one(pool)
    .await?;
    Ok(row.0)
}

// ─── Time-Series Queries ────────────────────────────────────────────────────

/// Daily transfer volume aggregated across all tokens.
/// Returns rows of (date, total_volume, transfer_count).
pub async fn get_daily_volume(
    pool: &PgPool,
    limit: i64,
) -> Result<Vec<(String, String, i64)>, sqlx::Error> {
    let rows: Vec<(String, String, i64)> = sqlx::query_as(
        r#"
        SELECT DATE(created_at)::TEXT AS day,
               COALESCE(SUM(CAST(amount AS NUMERIC)), 0)::TEXT AS volume,
               COUNT(*) AS tx_count
        FROM transfers
        GROUP BY DATE(created_at)
        ORDER BY day DESC
        LIMIT $1
        "#,
    )
    .bind(limit)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Monthly transfer volume aggregated across all tokens.
/// Returns rows of (month, total_volume, transfer_count).
pub async fn get_monthly_volume(
    pool: &PgPool,
    limit: i64,
) -> Result<Vec<(String, String, i64)>, sqlx::Error> {
    let rows: Vec<(String, String, i64)> = sqlx::query_as(
        r#"
        SELECT TO_CHAR(DATE_TRUNC('month', created_at), 'YYYY-MM') AS month,
               COALESCE(SUM(CAST(amount AS NUMERIC)), 0)::TEXT AS volume,
               COUNT(*) AS tx_count
        FROM transfers
        GROUP BY DATE_TRUNC('month', created_at)
        ORDER BY month DESC
        LIMIT $1
        "#,
    )
    .bind(limit)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Daily volume + activity for a specific token.
/// Returns rows of (date, volume, transfer_count).
pub async fn get_token_daily_volume(
    pool: &PgPool,
    token_address: &str,
    limit: i64,
) -> Result<Vec<(String, String, i64)>, sqlx::Error> {
    let rows: Vec<(String, String, i64)> = sqlx::query_as(
        r#"
        SELECT DATE(created_at)::TEXT AS day,
               COALESCE(SUM(CAST(amount AS NUMERIC)), 0)::TEXT AS volume,
               COUNT(*) AS tx_count
        FROM transfers
        WHERE token_address = $1
        GROUP BY DATE(created_at)
        ORDER BY day DESC
        LIMIT $2
        "#,
    )
    .bind(token_address)
    .bind(limit)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}
