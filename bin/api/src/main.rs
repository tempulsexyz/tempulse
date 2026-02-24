//! Tempulse API Server — serves stablecoin analytics from the indexed data.

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
};
use serde::{Deserialize, Serialize};
use std::{net::SocketAddr, sync::Arc};
use tempulse_core::{Settings, telemetry};
use tempulse_storage::{self as storage};

/// Shared application state.
struct AppState {
    pool: sqlx::PgPool,
}

#[tokio::main]
async fn main() {
    telemetry::init();
    let settings = Settings::from_env().expect("Failed to load settings");

    tracing::info!("Starting Tempulse API Server");

    // Connect to database
    let pool = storage::connect(&settings.database_url)
        .await
        .expect("Failed to connect to database");

    // Run migrations
    sqlx::migrate!("../../migrations")
        .run(&pool)
        .await
        .expect("Failed to run migrations");

    tracing::info!("Database ready");

    let state = Arc::new(AppState { pool });

    let app = Router::new()
        .route("/api/v1/tokens", get(list_tokens))
        .route("/api/v1/tokens/:address", get(get_token))
        .route("/api/v1/tokens/:address/holders", get(get_holders))
        .route(
            "/api/v1/tokens/:address/transfers",
            get(get_token_transfers),
        )
        .route("/api/v1/stats/volume", get(get_volume))
        .route("/api/v1/stats/overview", get(get_overview))
        .route("/api/v1/stats/daily", get(get_daily_volume))
        .route("/api/v1/stats/monthly", get(get_monthly_volume))
        .route(
            "/api/v1/tokens/:address/volume/daily",
            get(get_token_daily_volume),
        )
        .route("/api/v1/activity/recent", get(get_recent_activity))
        .route("/health", get(health))
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], settings.api_port));
    tracing::info!("Listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

// ─── Query Params ───────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct PaginationParams {
    limit: Option<i64>,
}

// ─── Response Types ─────────────────────────────────────────────────────────

#[derive(Serialize)]
struct ApiResponse<T: Serialize> {
    success: bool,
    data: T,
}

#[derive(Serialize)]
struct TokenVolumeEntry {
    token_address: String,
    symbol: String,
    total_volume: String,
    transfer_count: i64,
}

#[derive(Serialize)]
struct VolumeResponse {
    tokens: Vec<TokenVolumeEntry>,
}

#[derive(Serialize)]
struct OverviewResponse {
    total_value_transferred: String,
    total_transactions: i64,
    active_addresses: i64,
    tracked_tokens: i64,
}

fn json_ok<T: Serialize>(data: T) -> Json<ApiResponse<T>> {
    Json(ApiResponse {
        success: true,
        data,
    })
}

fn json_err(msg: &str) -> (StatusCode, Json<ApiResponse<String>>) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ApiResponse {
            success: false,
            data: msg.to_string(),
        }),
    )
}

// ─── Handlers ───────────────────────────────────────────────────────────────

async fn health() -> &'static str {
    "ok"
}

/// GET /api/v1/tokens — list tracked stablecoins (paginated).
async fn list_tokens(
    State(state): State<Arc<AppState>>,
    Query(params): Query<PaginationParams>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiResponse<String>>)> {
    let limit = params.limit.unwrap_or(100);
    let tokens = storage::repos::get_all_tokens(&state.pool, limit)
        .await
        .map_err(|e| json_err(&e.to_string()).into())?;
    Ok(json_ok(tokens))
}

/// GET /api/v1/tokens/:address — single token details.
async fn get_token(
    State(state): State<Arc<AppState>>,
    Path(address): Path<String>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiResponse<String>>)> {
    let token = storage::repos::get_token(&state.pool, &address)
        .await
        .map_err(|e| json_err(&e.to_string()).into())?;
    match token {
        Some(t) => Ok(json_ok(t)),
        None => Err(json_err("Token not found")),
    }
}

/// GET /api/v1/tokens/:address/holders — top holders for a token.
async fn get_holders(
    State(state): State<Arc<AppState>>,
    Path(address): Path<String>,
    Query(params): Query<PaginationParams>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiResponse<String>>)> {
    let limit = params.limit.unwrap_or(50);
    let holders = storage::repos::get_top_holders(&state.pool, &address, limit)
        .await
        .map_err(|e| json_err(&e.to_string()).into())?;
    Ok(json_ok(holders))
}

/// GET /api/v1/tokens/:address/transfers — transfers for a specific token.
async fn get_token_transfers(
    State(state): State<Arc<AppState>>,
    Path(address): Path<String>,
    Query(params): Query<PaginationParams>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiResponse<String>>)> {
    let limit = params.limit.unwrap_or(50);
    let transfers = storage::repos::get_token_transfers(&state.pool, &address, limit)
        .await
        .map_err(|e| json_err(&e.to_string()).into())?;
    Ok(json_ok(transfers))
}

/// GET /api/v1/stats/volume — per-token transfer volumes.
async fn get_volume(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiResponse<String>>)> {
    let rows = storage::repos::get_token_volumes(&state.pool)
        .await
        .map_err(|e| json_err(&e.to_string()).into())?;

    let tokens: Vec<TokenVolumeEntry> = rows
        .into_iter()
        .map(|(address, symbol, volume, count)| TokenVolumeEntry {
            token_address: address,
            symbol,
            total_volume: volume,
            transfer_count: count,
        })
        .collect();

    Ok(json_ok(VolumeResponse { tokens }))
}

/// GET /api/v1/stats/overview — global payment analytics.
async fn get_overview(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiResponse<String>>)> {
    let (total_volume, total_txs) = storage::repos::get_global_stats(&state.pool)
        .await
        .map_err(|e| json_err(&e.to_string()).into())?;
    let active_addrs = storage::repos::get_active_address_count(&state.pool)
        .await
        .map_err(|e| json_err(&e.to_string()).into())?;
    let token_count = storage::repos::get_token_count(&state.pool)
        .await
        .map_err(|e| json_err(&e.to_string()).into())?;

    Ok(json_ok(OverviewResponse {
        total_value_transferred: total_volume,
        total_transactions: total_txs,
        active_addresses: active_addrs,
        tracked_tokens: token_count,
    }))
}

async fn get_recent_activity(
    State(state): State<Arc<AppState>>,
    Query(params): Query<PaginationParams>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiResponse<String>>)> {
    let limit = params.limit.unwrap_or(50);
    let transfers = storage::repos::get_recent_transfers(&state.pool, limit)
        .await
        .map_err(|e| json_err(&e.to_string()).into())?;
    Ok(json_ok(transfers))
}

// ─── Time-Series Handlers ───────────────────────────────────────────────────

#[derive(Serialize)]
struct TimeSeriesEntry {
    date: String,
    volume: String,
    transfer_count: i64,
}

/// GET /api/v1/stats/daily — daily transfer volume (global).
async fn get_daily_volume(
    State(state): State<Arc<AppState>>,
    Query(params): Query<PaginationParams>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiResponse<String>>)> {
    let limit = params.limit.unwrap_or(90);
    let rows = storage::repos::get_daily_volume(&state.pool, limit)
        .await
        .map_err(|e| json_err(&e.to_string()).into())?;

    let entries: Vec<TimeSeriesEntry> = rows
        .into_iter()
        .map(|(date, volume, count)| TimeSeriesEntry {
            date,
            volume,
            transfer_count: count,
        })
        .collect();
    Ok(json_ok(entries))
}

/// GET /api/v1/stats/monthly — monthly transfer volume (global).
async fn get_monthly_volume(
    State(state): State<Arc<AppState>>,
    Query(params): Query<PaginationParams>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiResponse<String>>)> {
    let limit = params.limit.unwrap_or(24);
    let rows = storage::repos::get_monthly_volume(&state.pool, limit)
        .await
        .map_err(|e| json_err(&e.to_string()).into())?;

    let entries: Vec<TimeSeriesEntry> = rows
        .into_iter()
        .map(|(date, volume, count)| TimeSeriesEntry {
            date,
            volume,
            transfer_count: count,
        })
        .collect();
    Ok(json_ok(entries))
}

/// GET /api/v1/tokens/:address/volume/daily — daily volume for a specific token.
async fn get_token_daily_volume(
    State(state): State<Arc<AppState>>,
    Path(address): Path<String>,
    Query(params): Query<PaginationParams>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiResponse<String>>)> {
    let limit = params.limit.unwrap_or(90);
    let rows = storage::repos::get_token_daily_volume(&state.pool, &address, limit)
        .await
        .map_err(|e| json_err(&e.to_string()).into())?;

    let entries: Vec<TimeSeriesEntry> = rows
        .into_iter()
        .map(|(date, volume, count)| TimeSeriesEntry {
            date,
            volume,
            transfer_count: count,
        })
        .collect();
    Ok(json_ok(entries))
}
