//! Startup recovery for `saved_renders` rows left in `in_progress` after a
//! crash. Exercises `library::recover_orphaned_in_progress`.

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

use sl_map_web::library::recover_orphaned_in_progress;
use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

const USER_A: [u8; 16] = [1; 16];
const RENDER_IN_PROGRESS: [u8; 16] = [0x10; 16];
const RENDER_DONE: [u8; 16] = [0x11; 16];

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

async fn seed(pool: &SqlitePool) {
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
    // One in_progress and one done render, both owned personally by USER_A.
    for (rid, status) in [(RENDER_IN_PROGRESS, "in_progress"), (RENDER_DONE, "done")] {
        sqlx::query(
            "INSERT INTO saved_renders \
                (render_id, owner_user_id, owner_group_id, created_by, kind, status, \
                 settings_json, created_at) \
             VALUES (?1, ?2, NULL, ?3, 'grid_rectangle', ?4, '{}', ?5)",
        )
        .bind(rid.as_slice())
        .bind(USER_A.as_slice())
        .bind(USER_A.as_slice())
        .bind(status)
        .bind("2026-01-01")
        .execute(pool)
        .await
        .expect("insert render");
    }
}

#[tokio::test]
async fn recovery_marks_only_in_progress_rows_failed() {
    let pool = open_in_memory().await;
    seed(&pool).await;
    let count = recover_orphaned_in_progress(&pool)
        .await
        .expect("recovery succeeds");
    assert!(count == 1, "expected 1 row recovered, got {count}");

    // The previously-in_progress row is now failed with the canonical
    // message and a non-null finished_at.
    let (status, error_message, finished_at): (String, Option<String>, Option<String>) =
        sqlx::query_as(
            "SELECT status, error_message, finished_at \
             FROM saved_renders WHERE render_id = ?1",
        )
        .bind(RENDER_IN_PROGRESS.as_slice())
        .fetch_one(&pool)
        .await
        .expect("fetch in_progress row");
    assert!(status == "failed", "status: {status}");
    assert!(
        error_message
            .as_deref()
            .is_some_and(|m| m.contains("server restarted")),
        "error_message: {error_message:?}",
    );
    assert!(finished_at.is_some(), "finished_at should be set");

    // The originally-done row is untouched.
    let done_status: String =
        sqlx::query_scalar("SELECT status FROM saved_renders WHERE render_id = ?1")
            .bind(RENDER_DONE.as_slice())
            .fetch_one(&pool)
            .await
            .expect("fetch done row");
    assert!(done_status == "done", "done row was touched: {done_status}");
}

#[tokio::test]
async fn recovery_is_idempotent_on_clean_state() {
    let pool = open_in_memory().await;
    seed(&pool).await;
    let _first = recover_orphaned_in_progress(&pool)
        .await
        .expect("first pass");
    let second = recover_orphaned_in_progress(&pool)
        .await
        .expect("second pass");
    assert!(
        second == 0,
        "second pass should be a no-op, got {second} rows",
    );
}
