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
        .route("/api/v1/stats/tvl", get(get_tvl))
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
struct TvlEntry {
    token_address: String,
    symbol: String,
    total_supply: String,
}

#[derive(Serialize)]
struct TvlResponse {
    tokens: Vec<TvlEntry>,
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

/// GET /api/v1/tokens — list all tracked stablecoins.
async fn list_tokens(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiResponse<String>>)> {
    let tokens = storage::repos::get_all_tokens(&state.pool)
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

/// GET /api/v1/stats/tvl — total value locked across all tokens.
async fn get_tvl(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiResponse<String>>)> {
    let rows = storage::repos::get_tvl(&state.pool)
        .await
        .map_err(|e| json_err(&e.to_string()).into())?;

    let tokens: Vec<TvlEntry> = rows
        .into_iter()
        .map(|(address, symbol, supply)| TvlEntry {
            token_address: address,
            symbol,
            total_supply: supply,
        })
        .collect();

    Ok(json_ok(TvlResponse { tokens }))
}

/// GET /api/v1/activity/recent — latest transfers across all tokens.
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
