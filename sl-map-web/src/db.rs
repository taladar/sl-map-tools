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
/// pending migrations. Uses WAL journaling so reads do not block writes.
///
/// Migrations are run against a separate, single-connection pool with
/// `PRAGMA foreign_keys = OFF`. This is the recipe SQLite documents for
/// the ALTER TABLE pattern that rebuilds a parent table referenced by
/// other tables: `defer_foreign_keys` is not sufficient on its own
/// because the per-row deferred-check queue retains entries from the
/// moment a parent row "disappeared" (between the DROP and the RENAME)
/// and rejects them at COMMIT even when the new schema would have
/// accepted them. Since `PRAGMA foreign_keys` is a no-op inside a
/// transaction, the pragma has to be set on a connection that is not
/// already in a transaction — easiest to achieve by using a dedicated
/// pool just for migrations.
///
/// After the migrations run, we run `PRAGMA foreign_key_check` against
/// the same connection to verify the resulting schema does not contain
/// any orphaned FK references. The application's runtime pool is then
/// opened with FK enforcement back on so future inserts/updates are
/// checked normally.
///
/// # Errors
///
/// Returns a [`DbError`] if the URL is malformed, the file cannot be
/// opened, migrations fail, or the post-migration FK check finds
/// orphaned references.
pub async fn open_and_migrate(url: &str) -> Result<SqlitePool, DbError> {
    let migration_options = SqliteConnectOptions::from_str(url)
        .map_err(DbError::BadUrl)?
        .foreign_keys(false)
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
        .busy_timeout(std::time::Duration::from_secs(5));
    let migration_pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(migration_options)
        .await
        .map_err(DbError::Open)?;
    MIGRATIONS
        .run(&migration_pool)
        .await
        .map_err(DbError::Migrate)?;
    // Surface any orphaned FK references the migrations may have created
    // — a `foreign_key_check` row means a child row points at a parent
    // that does not exist, which is exactly the kind of inconsistency
    // running with `foreign_keys = OFF` would otherwise hide.
    let violations: Vec<(String, i64, String, i64)> = sqlx::query_as("PRAGMA foreign_key_check")
        .fetch_all(&migration_pool)
        .await
        .map_err(|err| DbError::Migrate(sqlx::migrate::MigrateError::Execute(err)))?;
    if !violations.is_empty() {
        tracing::error!(
            "post-migration foreign_key_check found {} violation(s): {:?}",
            violations.len(),
            violations
        );
        return Err(DbError::Migrate(sqlx::migrate::MigrateError::Execute(
            sqlx::Error::Protocol(format!(
                "post-migration foreign_key_check found {} violation(s)",
                violations.len()
            )),
        )));
    }
    migration_pool.close().await;

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
    Ok(pool)
}
