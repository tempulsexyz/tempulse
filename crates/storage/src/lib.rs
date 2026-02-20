pub mod models;
pub mod repos;

pub use sqlx::postgres::PgPool;
pub use sqlx::Pool;
pub use sqlx::Postgres;

pub async fn connect(database_url: &str) -> Result<PgPool, sqlx::Error> {
    PgPool::connect(database_url).await
}
