pub mod models;
pub mod repos;

pub use sqlx::Pool;
pub use sqlx::Postgres;
pub use sqlx::postgres::PgPool;

use sqlx::postgres::PgPoolOptions;
use std::time::Duration;

/// Connect to PostgreSQL with a production-ready connection pool.
pub async fn connect(database_url: &str) -> Result<PgPool, sqlx::Error> {
    PgPoolOptions::new()
        .max_connections(20)
        .min_connections(2)
        .acquire_timeout(Duration::from_secs(10))
        .idle_timeout(Duration::from_secs(300))
        .connect(database_url)
        .await
}
