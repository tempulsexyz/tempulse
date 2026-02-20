use sqlx::PgPool;

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

/// Insert a batch of transfers (skips duplicates on tx_hash + log_index).
pub async fn insert_transfers_batch(
    pool: &PgPool,
    transfers: &[NewTransfer],
) -> Result<(), sqlx::Error> {
    for t in transfers {
        sqlx::query(
            r#"
            INSERT INTO transfers (token_address, from_address, to_address, amount, memo, event_type, transaction_hash, block_number, log_index)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            ON CONFLICT (transaction_hash, log_index) DO NOTHING
            "#,
        )
        .bind(&t.token_address)
        .bind(&t.from_address)
        .bind(&t.to_address)
        .bind(&t.amount)
        .bind(&t.memo)
        .bind(&t.event_type)
        .bind(&t.transaction_hash)
        .bind(t.block_number)
        .bind(t.log_index)
        .execute(pool)
        .await?;
    }
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

// ─── Account Queries ────────────────────────────────────────────────────────

/// Upsert an account balance by adding a delta.
/// For mints: address = recipient, delta > 0.
/// For burns: address = sender, delta is subtracted.
/// For transfers: called twice — subtract from sender, add to receiver.
pub async fn upsert_account_balance(
    pool: &PgPool,
    address: &str,
    token_address: &str,
    amount: &str,
    is_add: bool,
    block_number: i64,
) -> Result<(), sqlx::Error> {
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
        .execute(pool)
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
        .execute(pool)
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
pub async fn insert_block(pool: &PgPool, block: &IndexedBlock) -> Result<(), sqlx::Error> {
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
    .execute(pool)
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
pub async fn delete_blocks_after(pool: &PgPool, block_number: i64) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM transfers WHERE block_number > $1")
        .bind(block_number)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM indexed_blocks WHERE block_number > $1")
        .bind(block_number)
        .execute(pool)
        .await?;
    Ok(())
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
pub async fn set_last_indexed_block(pool: &PgPool, block_number: i64) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE indexer_state SET value = $1 WHERE key = 'last_indexed_block'")
        .bind(block_number.to_string())
        .execute(pool)
        .await?;
    Ok(())
}

// ─── Analytics Queries ──────────────────────────────────────────────────────

/// Total Value Locked — sum of total_supply across all tokens.
/// Returns a list of (symbol, total_supply) pairs.
pub async fn get_tvl(pool: &PgPool) -> Result<Vec<(String, String, String)>, sqlx::Error> {
    let rows: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT address, symbol, total_supply FROM tokens ORDER BY CAST(total_supply AS NUMERIC) DESC",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}
