//! SQLite pool setup and migration runner.

use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::str::FromStr as _;

/// Embedded migrations. Resolved at compile time so the binary stays
/// self-contained and no separate migration command is needed.
pub static MIGRATIONS: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

/// Errors that can occur while opening the database.
#[derive(Debug, thiserror::Error)]
#[expect(
    clippy::module_name_repetitions,
    reason = "`DbError` is the conventional name and `Error` would clash with the crate's other top-level error type"
)]
pub enum DbError {
    /// the configured `database_url` could not be parsed as
    /// [`SqliteConnectOptions`].
    #[error("invalid database URL: {0}")]
    BadUrl(#[source] sqlx::Error),
    /// the pool could not be opened (file permissions, locked, etc.).
    #[error("failed to open SQLite pool: {0}")]
    Open(#[source] sqlx::Error),
    /// applying the embedded migrations failed.
    #[error("failed to apply migrations: {0}")]
    Migrate(#[source] sqlx::migrate::MigrateError),
}

/// Open the SQLite pool from a `sqlx` connection URL and apply any
/// pending migrations. Enables foreign keys (off by default in SQLite) and
/// uses WAL journaling so reads do not block writes.
///
/// # Errors
///
/// Returns a [`DbError`] if the URL is malformed, the file cannot be
/// opened, or migrations fail.
pub async fn open_and_migrate(url: &str) -> Result<SqlitePool, DbError> {
    let options = SqliteConnectOptions::from_str(url)
        .map_err(DbError::BadUrl)?
        .foreign_keys(true)
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
        .busy_timeout(std::time::Duration::from_secs(5));
    let pool = SqlitePoolOptions::new()
        .max_connections(8)
        .connect_with(options)
        .await
        .map_err(DbError::Open)?;
    MIGRATIONS.run(&pool).await.map_err(DbError::Migrate)?;
    Ok(pool)
}
