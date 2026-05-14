//! DB-level enforcement that every `saved_renders.notecard_id` reference
//! shares the render's scope. Exercises the three triggers added in
//! migration `0005_notecard_scope_match.sql`.

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

use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

const USER_A: [u8; 16] = [1; 16];
const GROUP_A: [u8; 16] = [0xAA; 16];
const GROUP_B: [u8; 16] = [0xBB; 16];
const NC_IN_A: [u8; 16] = [0xCC; 16];
const RENDER_IN_A: [u8; 16] = [0x10; 16];
const RENDER_IN_B: [u8; 16] = [0x20; 16];

async fn open_in_memory() -> SqlitePool {
    let options = SqliteConnectOptions::from_str("sqlite::memory:")
        .expect("static in-memory URL parses")
        .foreign_keys(true);
    // max_connections = 1 so every query lands on the same in-memory DB.
    // A larger pool would silently give each test its own empty DB after
    // the migrator ran on whichever connection it grabbed first.
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
    for (gid, name) in [(GROUP_A, "GroupA"), (GROUP_B, "GroupB")] {
        sqlx::query(
            "INSERT INTO \"groups\" (group_id, name, created_by, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?4)",
        )
        .bind(gid.as_slice())
        .bind(name)
        .bind(USER_A.as_slice())
        .bind("2026-01-01")
        .execute(pool)
        .await
        .expect("insert group");
        sqlx::query(
            "INSERT INTO group_memberships (group_id, user_id, role, created_at) \
             VALUES (?1, ?2, 'owner', ?3)",
        )
        .bind(gid.as_slice())
        .bind(USER_A.as_slice())
        .bind("2026-01-01")
        .execute(pool)
        .await
        .expect("insert membership");
    }
    sqlx::query(
        "INSERT INTO saved_notecards \
            (notecard_id, owner_user_id, owner_group_id, uploaded_by, name, body, created_at) \
         VALUES (?1, NULL, ?2, ?3, ?4, ?5, ?6)",
    )
    .bind(NC_IN_A.as_slice())
    .bind(GROUP_A.as_slice())
    .bind(USER_A.as_slice())
    .bind("nc-in-A")
    .bind("body")
    .bind("2026-01-01")
    .execute(pool)
    .await
    .expect("insert notecard");
}

async fn insert_render(
    pool: &SqlitePool,
    render_id: [u8; 16],
    owner_group: [u8; 16],
    notecard: [u8; 16],
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO saved_renders \
            (render_id, owner_user_id, owner_group_id, created_by, notecard_id, \
             kind, settings_json, created_at) \
         VALUES (?1, NULL, ?2, ?3, ?4, 'usb_notecard', '{}', ?5)",
    )
    .bind(render_id.as_slice())
    .bind(owner_group.as_slice())
    .bind(USER_A.as_slice())
    .bind(notecard.as_slice())
    .bind("2026-01-01")
    .execute(pool)
    .await
    .map(drop)
}

#[tokio::test]
async fn same_scope_render_insert_succeeds() {
    let pool = open_in_memory().await;
    seed(&pool).await;
    insert_render(&pool, RENDER_IN_A, GROUP_A, NC_IN_A)
        .await
        .expect("same-scope insert must succeed");
}

#[tokio::test]
async fn cross_scope_render_insert_rejected() {
    let pool = open_in_memory().await;
    seed(&pool).await;
    let err = insert_render(&pool, RENDER_IN_B, GROUP_B, NC_IN_A)
        .await
        .expect_err("cross-scope insert must abort");
    let msg = err.to_string();
    assert!(
        msg.contains("render and notecard must share the same scope"),
        "unexpected error: {msg}",
    );
}

#[tokio::test]
async fn group_delete_cascades_when_refs_are_same_scope() {
    let pool = open_in_memory().await;
    seed(&pool).await;
    insert_render(&pool, RENDER_IN_A, GROUP_A, NC_IN_A)
        .await
        .expect("same-scope insert must succeed");
    sqlx::query("DELETE FROM \"groups\" WHERE group_id = ?1")
        .bind(GROUP_A.as_slice())
        .execute(&pool)
        .await
        .expect("group delete must succeed");
    let render_count: i64 = sqlx::query_scalar("SELECT count(*) FROM saved_renders")
        .fetch_one(&pool)
        .await
        .expect("count renders");
    let notecard_count: i64 = sqlx::query_scalar("SELECT count(*) FROM saved_notecards")
        .fetch_one(&pool)
        .await
        .expect("count notecards");
    assert!(render_count == 0, "renders not cleaned up: {render_count}");
    assert!(
        notecard_count == 0,
        "notecards not cleaned up: {notecard_count}"
    );
}

#[tokio::test]
async fn rescoping_referenced_notecard_rejected() {
    let pool = open_in_memory().await;
    seed(&pool).await;
    insert_render(&pool, RENDER_IN_A, GROUP_A, NC_IN_A)
        .await
        .expect("same-scope insert must succeed");
    let err = sqlx::query(
        "UPDATE saved_notecards SET owner_user_id = ?1, owner_group_id = NULL \
         WHERE notecard_id = ?2",
    )
    .bind(USER_A.as_slice())
    .bind(NC_IN_A.as_slice())
    .execute(&pool)
    .await
    .expect_err("re-scope of referenced notecard must abort");
    let msg = err.to_string();
    assert!(
        msg.contains("cannot re-scope a notecard referenced by a render"),
        "unexpected error: {msg}",
    );
}
