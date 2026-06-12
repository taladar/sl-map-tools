//! DB-level invariants for `saved_logos` and `saved_render_logos`
//! (migration `0011_saved_logos.sql`): the Personal-or-Group XOR ownership
//! check, and the `ON DELETE RESTRICT` on a logo referenced by a render plus
//! the `ON DELETE CASCADE` of the link rows when the render is deleted.

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
const GROUP_A: [u8; 16] = [0xAA; 16];
const LOGO_PERSONAL: [u8; 16] = [0xC1; 16];
const LOGO_GROUP: [u8; 16] = [0xC2; 16];
const RENDER_A: [u8; 16] = [0x10; 16];

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
    sqlx::query(
        "INSERT INTO \"groups\" (group_id, name, created_by, created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, ?4)",
    )
    .bind(GROUP_A.as_slice())
    .bind("GroupA")
    .bind(USER_A.as_slice())
    .bind("2026-01-01")
    .execute(pool)
    .await
    .expect("insert group");
}

async fn insert_personal_logo(pool: &SqlitePool, logo_id: &[u8]) {
    sqlx::query(
        "INSERT INTO saved_logos \
            (logo_id, owner_user_id, owner_group_id, uploaded_by, name, \
             content_type, image_filename, width, height, byte_size, created_at) \
         VALUES (?1, ?2, NULL, ?2, 'Logo', 'image/png', ?3, 64, 64, 100, '2026-01-01')",
    )
    .bind(logo_id)
    .bind(USER_A.as_slice())
    .bind(format!(
        "{}.png",
        uuid::Uuid::from_slice(logo_id).expect("uuid")
    ))
    .execute(pool)
    .await
    .expect("insert personal logo");
}

async fn insert_personal_render(pool: &SqlitePool) {
    sqlx::query(
        "INSERT INTO saved_renders \
            (render_id, owner_user_id, owner_group_id, created_by, kind, status, \
             settings_json, created_at) \
         VALUES (?1, ?2, NULL, ?2, 'grid_rectangle', 'done', '{}', '2026-01-01')",
    )
    .bind(RENDER_A.as_slice())
    .bind(USER_A.as_slice())
    .execute(pool)
    .await
    .expect("insert render");
}

#[tokio::test]
async fn xor_ownership_rejects_both_and_neither() {
    let pool = open_in_memory().await;
    seed(&pool).await;

    // Both owners set -> CHECK violation.
    let both = sqlx::query(
        "INSERT INTO saved_logos \
            (logo_id, owner_user_id, owner_group_id, uploaded_by, name, \
             content_type, image_filename, width, height, byte_size, created_at) \
         VALUES (?1, ?2, ?3, ?2, 'x', 'image/png', 'a.png', 1, 1, 1, '2026-01-01')",
    )
    .bind([0xD1; 16].as_slice())
    .bind(USER_A.as_slice())
    .bind(GROUP_A.as_slice())
    .execute(&pool)
    .await;
    assert!(both.is_err(), "both owners set must violate the XOR CHECK");

    // Neither owner set -> CHECK violation.
    let neither = sqlx::query(
        "INSERT INTO saved_logos \
            (logo_id, owner_user_id, owner_group_id, uploaded_by, name, \
             content_type, image_filename, width, height, byte_size, created_at) \
         VALUES (?1, NULL, NULL, ?2, 'x', 'image/png', 'a.png', 1, 1, 1, '2026-01-01')",
    )
    .bind([0xD2; 16].as_slice())
    .bind(USER_A.as_slice())
    .execute(&pool)
    .await;
    assert!(
        neither.is_err(),
        "neither owner set must violate the XOR CHECK"
    );
}

#[tokio::test]
async fn group_owned_logo_inserts() {
    let pool = open_in_memory().await;
    seed(&pool).await;
    sqlx::query(
        "INSERT INTO saved_logos \
            (logo_id, owner_user_id, owner_group_id, uploaded_by, name, \
             content_type, image_filename, width, height, byte_size, created_at) \
         VALUES (?1, NULL, ?2, ?3, 'g', 'image/webp', 'g.webp', 32, 32, 50, '2026-01-01')",
    )
    .bind(LOGO_GROUP.as_slice())
    .bind(GROUP_A.as_slice())
    .bind(USER_A.as_slice())
    .execute(&pool)
    .await
    .expect("group-owned logo inserts");
}

#[tokio::test]
async fn referenced_logo_cannot_be_deleted_then_cascade_frees_it() {
    let pool = open_in_memory().await;
    seed(&pool).await;
    insert_personal_logo(&pool, LOGO_PERSONAL.as_slice()).await;
    insert_personal_render(&pool).await;
    sqlx::query("INSERT INTO saved_render_logos (render_id, logo_id) VALUES (?1, ?2)")
        .bind(RENDER_A.as_slice())
        .bind(LOGO_PERSONAL.as_slice())
        .execute(&pool)
        .await
        .expect("link render to logo");

    // RESTRICT: the logo is referenced, so deleting it must fail.
    let blocked = sqlx::query("DELETE FROM saved_logos WHERE logo_id = ?1")
        .bind(LOGO_PERSONAL.as_slice())
        .execute(&pool)
        .await;
    assert!(
        blocked.is_err(),
        "a logo referenced by a render must not be deletable"
    );

    // Deleting the render cascades the link row away.
    sqlx::query("DELETE FROM saved_renders WHERE render_id = ?1")
        .bind(RENDER_A.as_slice())
        .execute(&pool)
        .await
        .expect("delete render");
    let links: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM saved_render_logos WHERE logo_id = ?1")
            .bind(LOGO_PERSONAL.as_slice())
            .fetch_one(&pool)
            .await
            .expect("count links");
    assert_eq!(links, 0, "deleting the render cascades the link rows");

    // Now the logo deletes cleanly.
    sqlx::query("DELETE FROM saved_logos WHERE logo_id = ?1")
        .bind(LOGO_PERSONAL.as_slice())
        .execute(&pool)
        .await
        .expect("logo deletes once unreferenced");
}

#[tokio::test]
async fn deleting_user_cascades_personal_logo() {
    let pool = open_in_memory().await;
    seed(&pool).await;
    insert_personal_logo(&pool, LOGO_PERSONAL.as_slice()).await;
    sqlx::query("DELETE FROM users WHERE user_id = ?1")
        .bind(USER_A.as_slice())
        .execute(&pool)
        .await
        .expect("delete user");
    let remaining: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM saved_logos")
        .fetch_one(&pool)
        .await
        .expect("count logos");
    assert_eq!(
        remaining, 0,
        "a personal logo is removed when its owner is deleted (ON DELETE CASCADE)"
    );
}
