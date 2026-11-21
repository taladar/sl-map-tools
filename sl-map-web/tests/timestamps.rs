//! Timestamp storage and serialization after the chrono → time migration.
//!
//! Rows written by older releases (chrono-based sqlx encoding) carry a
//! `+00:00` suffix, new rows carry `Z`. These tests pin down that both
//! decode, that migration 0014 normalises the legacy suffix, and that the
//! JSON shape the frontend sees stays RFC 3339 with `Z`.

#![allow(
    clippy::expect_used,
    reason = "test code panics on assert failure for clearer failure output"
)]
#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers do not need doc comments"
)]
#![allow(
    clippy::tests_outside_test_module,
    reason = "integration tests live in tests/, not in a #[cfg(test)] module"
)]

use std::str::FromStr as _;

use pretty_assertions::assert_eq;

use sl_map_web::routes::users::UserProfile;
use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use time::OffsetDateTime;
use time::macros::datetime;
use uuid::Uuid;

const USER_A: [u8; 16] = [1; 16];
const USER_B: [u8; 16] = [2; 16];

/// The normalising migration, re-run explicitly against rows inserted after
/// the migrator has already applied it once.
const MIGRATION_0014: &str = include_str!("../migrations/0014_timestamps_rfc3339_z.sql");

async fn open_in_memory() -> SqlitePool {
    let options = SqliteConnectOptions::from_str("sqlite::memory:")
        .expect("static in-memory URL parses")
        .foreign_keys(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .expect("open in-memory pool");
    sl_map_web::db::MIGRATIONS
        .run(&pool)
        .await
        .expect("apply migrations");
    pool
}

async fn insert_user_raw(pool: &SqlitePool, user_id: [u8; 16], name: &str, ts: &str) {
    sqlx::query(
        "INSERT INTO users (user_id, legacy_name, username, created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, ?4)",
    )
    .bind(user_id.as_slice())
    .bind(name)
    .bind(name.to_lowercase().replace(' ', "."))
    .bind(ts)
    .execute(pool)
    .await
    .expect("insert user");
}

async fn created_at_text(pool: &SqlitePool, user_id: [u8; 16]) -> String {
    sqlx::query_scalar("SELECT created_at FROM users WHERE user_id = ?1")
        .bind(user_id.as_slice())
        .fetch_one(pool)
        .await
        .expect("select created_at text")
}

async fn created_at_decoded(pool: &SqlitePool, user_id: [u8; 16]) -> OffsetDateTime {
    sqlx::query_scalar("SELECT created_at FROM users WHERE user_id = ?1")
        .bind(user_id.as_slice())
        .fetch_one(pool)
        .await
        .expect("select created_at decoded")
}

#[tokio::test]
async fn new_timestamps_are_stored_as_rfc3339_z_and_round_trip() {
    let pool = open_in_memory().await;
    let ts = datetime!(2026-11-21 10:11:12.345 UTC);
    sqlx::query(
        "INSERT INTO users (user_id, legacy_name, username, created_at, updated_at) \
         VALUES (?1, 'Alice One', 'alice.one', ?2, ?2)",
    )
    .bind(USER_A.as_slice())
    .bind(ts)
    .execute(&pool)
    .await
    .expect("insert user");

    assert_eq!(
        created_at_text(&pool, USER_A).await,
        "2026-11-21T10:11:12.345Z"
    );
    assert_eq!(created_at_decoded(&pool, USER_A).await, ts);
}

#[tokio::test]
async fn legacy_chrono_timestamps_still_decode() {
    let pool = open_in_memory().await;
    insert_user_raw(&pool, USER_A, "Alice One", "2026-01-02T03:04:05.678+00:00").await;
    insert_user_raw(&pool, USER_B, "Bob Two", "2026-01-02T03:04:05+00:00").await;

    assert_eq!(
        created_at_decoded(&pool, USER_A).await,
        datetime!(2026-01-02 03:04:05.678 UTC)
    );
    assert_eq!(
        created_at_decoded(&pool, USER_B).await,
        datetime!(2026-01-02 03:04:05 UTC)
    );
}

#[tokio::test]
async fn migration_0014_rewrites_legacy_suffix_only() {
    let pool = open_in_memory().await;
    insert_user_raw(&pool, USER_A, "Alice One", "2026-01-02T03:04:05.678+00:00").await;
    insert_user_raw(&pool, USER_B, "Bob Two", "2026-01-02T03:04:05.9Z").await;

    sqlx::raw_sql(MIGRATION_0014)
        .execute(&pool)
        .await
        .expect("re-run migration 0014");

    assert_eq!(
        created_at_text(&pool, USER_A).await,
        "2026-01-02T03:04:05.678Z"
    );
    assert_eq!(
        created_at_text(&pool, USER_B).await,
        "2026-01-02T03:04:05.9Z",
        "already-normalised rows must be left untouched"
    );
    assert_eq!(
        created_at_decoded(&pool, USER_A).await,
        datetime!(2026-01-02 03:04:05.678 UTC)
    );
}

#[test]
fn json_timestamps_keep_the_rfc3339_z_shape() {
    let profile = UserProfile {
        user_id: Uuid::from_bytes(USER_A),
        username: "alice.one".to_owned(),
        legacy_name: "Alice One".to_owned(),
        created_at: datetime!(2026-11-21 10:11:12.345 UTC),
    };
    let json = serde_json::to_value(&profile).expect("serialize profile");
    assert_eq!(
        json.get("created_at").and_then(|v| v.as_str()),
        Some("2026-11-21T10:11:12.345Z")
    );
}
