-- Tempulse: Initial Schema
-- Creates all core tables for the stablecoin analytics platform.

-- ─── Indexed Blocks (reorg tracking) ────────────────────────────────────────
CREATE TABLE IF NOT EXISTS indexed_blocks (
    block_number  BIGINT PRIMARY KEY,
    block_hash    TEXT NOT NULL,
    parent_hash   TEXT NOT NULL,
    timestamp     BIGINT NOT NULL
);

-- ─── Tracked Tokens ─────────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS tokens (
    address          TEXT PRIMARY KEY,
    name             TEXT NOT NULL,
    symbol           TEXT NOT NULL,
    decimals         SMALLINT NOT NULL DEFAULT 6,
    currency         TEXT NOT NULL DEFAULT '',
    total_supply     TEXT NOT NULL DEFAULT '0',
    created_at_block BIGINT NOT NULL DEFAULT 0,
    created_at_tx    TEXT NOT NULL DEFAULT ''
);

-- ─── Transfers / Mints / Burns ──────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS transfers (
    id                BIGSERIAL PRIMARY KEY,
    token_address     TEXT NOT NULL REFERENCES tokens(address),
    from_address      TEXT NOT NULL,
    to_address        TEXT NOT NULL,
    amount            TEXT NOT NULL,
    memo              TEXT,
    event_type        TEXT NOT NULL DEFAULT 'transfer',  -- transfer | mint | burn
    transaction_hash  TEXT NOT NULL,
    block_number      BIGINT NOT NULL,
    log_index         INTEGER NOT NULL,
    created_at        TIMESTAMP NOT NULL DEFAULT NOW(),
    UNIQUE (transaction_hash, log_index)
);

CREATE INDEX IF NOT EXISTS idx_transfers_token ON transfers(token_address);
CREATE INDEX IF NOT EXISTS idx_transfers_block ON transfers(block_number);
CREATE INDEX IF NOT EXISTS idx_transfers_from  ON transfers(from_address);
CREATE INDEX IF NOT EXISTS idx_transfers_to    ON transfers(to_address);

-- ─── Account Balances ───────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS accounts (
    address          TEXT NOT NULL,
    token_address    TEXT NOT NULL REFERENCES tokens(address),
    balance          TEXT NOT NULL DEFAULT '0',
    updated_at_block BIGINT NOT NULL DEFAULT 0,
    PRIMARY KEY (address, token_address)
);

CREATE INDEX IF NOT EXISTS idx_accounts_token ON accounts(token_address);

-- ─── Hourly Stats (aggregated analytics) ────────────────────────────────────
CREATE TABLE IF NOT EXISTS hourly_stats (
    token_address    TEXT NOT NULL REFERENCES tokens(address),
    hour             TIMESTAMP NOT NULL,
    transfer_count   BIGINT NOT NULL DEFAULT 0,
    transfer_volume  TEXT NOT NULL DEFAULT '0',
    mint_count       BIGINT NOT NULL DEFAULT 0,
    mint_volume      TEXT NOT NULL DEFAULT '0',
    burn_count       BIGINT NOT NULL DEFAULT 0,
    burn_volume      TEXT NOT NULL DEFAULT '0',
    unique_senders   BIGINT NOT NULL DEFAULT 0,
    unique_receivers BIGINT NOT NULL DEFAULT 0,
    PRIMARY KEY (token_address, hour)
);

-- ─── Indexer State ──────────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS indexer_state (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

-- Seed the initial state
INSERT INTO indexer_state (key, value)
VALUES ('last_indexed_block', '0')
ON CONFLICT (key) DO NOTHING;
