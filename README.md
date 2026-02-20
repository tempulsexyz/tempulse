# Tempulse

**Tempulse** is a high-performance stablecoin analytics platform for the [Tempo](https://tempo.xyz) blockchain. It crawls TIP-20 stablecoin events, indexes them into PostgreSQL, and serves analytics via a REST API.

## Architecture

```
bin/fin   → Indexer daemon (crawls blocks, decodes events, writes to DB)
bin/api   → REST API server (reads from DB, serves analytics)
crates/core    → Config, errors, telemetry
crates/tempo   → TIP-20/Factory ABIs, log decoders, RPC provider
crates/storage → PostgreSQL models, repositories, migrations
```

## Quick Start

### 1. Start PostgreSQL

```bash
docker-compose up -d
```

### 2. Configure

```bash
cp .env.example .env
# Edit .env if needed (defaults work for local development)
```

### 3. Run the Indexer

```bash
cargo run --bin fin
```

The indexer will:
- Discover TIP-20 tokens from the Factory contract
- Index all Transfer/Mint/Burn events
- Track account balances in real-time
- Persist progress and resume from where it left off

### 4. Run the API

```bash
cargo run --bin api
```

### API Endpoints

| Endpoint | Description |
|---|---|
| `GET /api/v1/tokens` | List all tracked stablecoins |
| `GET /api/v1/tokens/:address` | Single token details |
| `GET /api/v1/tokens/:address/holders` | Top holders with balances |
| `GET /api/v1/tokens/:address/transfers` | Token transfer history |
| `GET /api/v1/stats/tvl` | Total Value Locked |
| `GET /api/v1/activity/recent` | Latest transfers |
| `GET /health` | Health check |

## Environment Variables

| Variable | Default | Description |
|---|---|---|
| `DATABASE_URL` | `postgres://tempulse:tempulse@localhost:5432/tempulse` | PostgreSQL connection |
| `RPC_URL` | `https://rpc.moderato.tempo.xyz` | Tempo RPC endpoint |
| `START_BLOCK` | `0` | Block to start indexing from |
| `BATCH_SIZE` | `100` | Blocks per indexing batch |
| `API_PORT` | `3000` | API server port |
| `RUST_LOG` | `info` | Log level |

## Development

```bash
cargo build          # Build all
cargo test           # Run tests
cargo clippy         # Lint
```
