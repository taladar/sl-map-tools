//! `auth::delete_session` removes exactly the row whose
//! `session_id_hash` matches `SHA-256(raw_id)`. This is the underlying
//! machinery the L17 login-rotation depends on (and that logout has
//! always relied on).

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

use sha2::{Digest as _, Sha256};
use sl_map_web::auth::delete_session;
use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

const USER_A: [u8; 16] = [1; 16];
const RAW_SESSION_PRESENT: [u8; 32] = [0xAA; 32];
const RAW_SESSION_ABSENT: [u8; 32] = [0xBB; 32];
const RAW_SESSION_OTHER: [u8; 32] = [0xCC; 32];

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

async fn seed_user(pool: &SqlitePool) {
    sqlx::query(
        "INSERT INTO users (user_id, legacy_name, username, created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, ?4)",
    )
    .bind(USER_A.as_slice())
    .bind("Alice One")
    .bind("alice.one")
    .bind("2026-01-01")
    .execute(pool)
    .await
    .expect("insert user");
}

async fn insert_session(pool: &SqlitePool, raw: &[u8; 32]) {
    let hash = Sha256::digest(raw).to_vec();
    sqlx::query(
        "INSERT INTO sessions \
            (session_id_hash, user_id, expires_at, created_at, last_seen_at, client_ip) \
         VALUES (?1, ?2, ?3, ?4, ?4, NULL)",
    )
    .bind(hash)
    .bind(USER_A.as_slice())
    .bind("2099-01-01")
    .bind("2026-01-01")
    .execute(pool)
    .await
    .expect("insert session");
}

async fn count_sessions(pool: &SqlitePool, raw: &[u8; 32]) -> i64 {
    let hash = Sha256::digest(raw).to_vec();
    sqlx::query_scalar("SELECT count(*) FROM sessions WHERE session_id_hash = ?1")
        .bind(hash)
        .fetch_one(pool)
        .await
        .expect("count sessions")
}

#[tokio::test]
async fn delete_session_removes_matching_row_only() {
    let pool = open_in_memory().await;
    seed_user(&pool).await;
    insert_session(&pool, &RAW_SESSION_PRESENT).await;
    insert_session(&pool, &RAW_SESSION_OTHER).await;

    delete_session(&pool, &RAW_SESSION_PRESENT)
        .await
        .expect("delete_session");

    assert!(
        count_sessions(&pool, &RAW_SESSION_PRESENT).await == 0,
        "targeted session was not removed",
    );
    assert!(
        count_sessions(&pool, &RAW_SESSION_OTHER).await == 1,
        "unrelated session was wrongly removed",
    );
}

#[tokio::test]
async fn delete_session_is_a_noop_for_unknown_id() {
    let pool = open_in_memory().await;
    seed_user(&pool).await;
    insert_session(&pool, &RAW_SESSION_OTHER).await;

    // Deleting an id that was never inserted must not error and must not
    // affect any existing rows.
    delete_session(&pool, &RAW_SESSION_ABSENT)
        .await
        .expect("delete_session on absent id should be a no-op");

    assert!(
        count_sessions(&pool, &RAW_SESSION_OTHER).await == 1,
        "no-op delete touched an unrelated row",
    );
}
