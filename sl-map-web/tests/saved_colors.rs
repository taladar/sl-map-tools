//! DB-level invariants for the `saved_colors` palette table (migration
//! `0012_saved_custom_colors.sql`): idempotent saves via the
//! `(user_id, color)` primary key, per-user isolation, and the
//! `ON DELETE CASCADE` that clears a user's palette when the account is
//! deleted. The `#rrggbb` format is enforced at the API surface (see the
//! `is_canonical_hex_color` unit tests in `src/routes/users.rs`), not by
//! the schema, so it is not exercised here.

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
use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

const USER_A: [u8; 16] = [1; 16];
const USER_B: [u8; 16] = [2; 16];

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

async fn insert_user(pool: &SqlitePool, user_id: &[u8], username: &str) {
    sqlx::query(
        "INSERT INTO users (user_id, legacy_name, username, created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, ?4)",
    )
    .bind(user_id)
    .bind(username)
    .bind(username)
    .bind("2026-01-01")
    .execute(pool)
    .await
    .expect("insert user");
}

async fn add_color(pool: &SqlitePool, user_id: &[u8], color: &str) -> u64 {
    sqlx::query(
        "INSERT OR IGNORE INTO saved_colors (user_id, color, created_at) VALUES (?1, ?2, ?3)",
    )
    .bind(user_id)
    .bind(color)
    .bind("2026-01-01")
    .execute(pool)
    .await
    .expect("insert saved color")
    .rows_affected()
}

async fn colors_for(pool: &SqlitePool, user_id: &[u8]) -> Vec<String> {
    sqlx::query_scalar(
        "SELECT color FROM saved_colors WHERE user_id = ?1 ORDER BY created_at, color",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
    .expect("list colors")
}

#[tokio::test]
async fn saving_the_same_colour_twice_is_idempotent() {
    let pool = open_in_memory().await;
    insert_user(&pool, USER_A.as_slice(), "alice.one").await;

    let first = add_color(&pool, USER_A.as_slice(), "#ff0000").await;
    let second = add_color(&pool, USER_A.as_slice(), "#ff0000").await;
    assert_eq!(first, 1, "the first save inserts a row");
    assert_eq!(second, 0, "INSERT OR IGNORE absorbs the duplicate");
    assert_eq!(
        colors_for(&pool, USER_A.as_slice()).await,
        vec!["#ff0000".to_owned()],
        "the colour appears exactly once",
    );
}

#[tokio::test]
async fn palettes_are_per_user() {
    let pool = open_in_memory().await;
    insert_user(&pool, USER_A.as_slice(), "alice.one").await;
    insert_user(&pool, USER_B.as_slice(), "bob.two").await;

    add_color(&pool, USER_A.as_slice(), "#ff0000").await;
    add_color(&pool, USER_B.as_slice(), "#00ff00").await;

    assert_eq!(
        colors_for(&pool, USER_A.as_slice()).await,
        vec!["#ff0000".to_owned()],
        "user A sees only their own colour",
    );
    assert_eq!(
        colors_for(&pool, USER_B.as_slice()).await,
        vec!["#00ff00".to_owned()],
        "user B sees only their own colour",
    );
}

#[tokio::test]
async fn deleting_a_user_clears_their_palette() {
    let pool = open_in_memory().await;
    insert_user(&pool, USER_A.as_slice(), "alice.one").await;
    insert_user(&pool, USER_B.as_slice(), "bob.two").await;
    add_color(&pool, USER_A.as_slice(), "#ff0000").await;
    add_color(&pool, USER_A.as_slice(), "#123456").await;
    add_color(&pool, USER_B.as_slice(), "#00ff00").await;

    sqlx::query("DELETE FROM users WHERE user_id = ?1")
        .bind(USER_A.as_slice())
        .execute(&pool)
        .await
        .expect("delete user A");

    assert!(
        colors_for(&pool, USER_A.as_slice()).await.is_empty(),
        "user A's palette is cascaded away",
    );
    assert_eq!(
        colors_for(&pool, USER_B.as_slice()).await,
        vec!["#00ff00".to_owned()],
        "user B's palette is untouched",
    );
}
