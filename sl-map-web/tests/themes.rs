//! DB-level invariants for the `themes` table (migration `0013_themes.sql`):
//! the Personal-or-Group XOR CHECK, per-user and per-group isolation, the
//! `ON DELETE CASCADE` from both owner columns, and the `created_by`
//! `ON DELETE SET NULL` that keeps a group theme alive after its creator
//! deletes their account. Colour-format validation is enforced at the API
//! surface (see the `ThemeSettings::validate` unit tests in
//! `src/routes/themes.rs`), not by the schema, so it is not exercised here.

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
const GROUP_G: [u8; 16] = [9; 16];
const THEME_1: [u8; 16] = [11; 16];
const THEME_2: [u8; 16] = [12; 16];

const SETTINGS: &str = r#"{"version":1,"draw_region_names":true}"#;

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

async fn insert_group(pool: &SqlitePool, group_id: &[u8], created_by: &[u8], name: &str) {
    sqlx::query(
        "INSERT INTO \"groups\" (group_id, name, created_by, created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, ?4)",
    )
    .bind(group_id)
    .bind(name)
    .bind(created_by)
    .bind("2026-01-01")
    .execute(pool)
    .await
    .expect("insert group");
}

async fn insert_membership(pool: &SqlitePool, group_id: &[u8], user_id: &[u8], role: &str) {
    sqlx::query(
        "INSERT INTO group_memberships (group_id, user_id, role, created_at) \
         VALUES (?1, ?2, ?3, ?4)",
    )
    .bind(group_id)
    .bind(user_id)
    .bind(role)
    .bind("2026-01-01")
    .execute(pool)
    .await
    .expect("insert membership");
}

/// Insert a theme directly. Exactly one of `owner_user_id` / `owner_group_id`
/// should be `Some` for a valid row; the tests pass other combinations on
/// purpose to exercise the CHECK constraint.
async fn insert_theme(
    pool: &SqlitePool,
    theme_id: &[u8],
    owner_user_id: Option<&[u8]>,
    owner_group_id: Option<&[u8]>,
    created_by: &[u8],
    name: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO themes \
            (theme_id, owner_user_id, owner_group_id, created_by, name, \
             settings_json, created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)",
    )
    .bind(theme_id)
    .bind(owner_user_id)
    .bind(owner_group_id)
    .bind(created_by)
    .bind(name)
    .bind(SETTINGS)
    .bind("2026-01-01")
    .execute(pool)
    .await
    .map(|_| ())
}

async fn personal_theme_names(pool: &SqlitePool, user_id: &[u8]) -> Vec<String> {
    sqlx::query_scalar("SELECT name FROM themes WHERE owner_user_id = ?1 ORDER BY created_at, name")
        .bind(user_id)
        .fetch_all(pool)
        .await
        .expect("list personal themes")
}

async fn group_theme_names(pool: &SqlitePool, group_id: &[u8]) -> Vec<String> {
    sqlx::query_scalar(
        "SELECT name FROM themes WHERE owner_group_id = ?1 ORDER BY created_at, name",
    )
    .bind(group_id)
    .fetch_all(pool)
    .await
    .expect("list group themes")
}

#[tokio::test]
async fn xor_check_rejects_both_and_neither_owner() {
    let pool = open_in_memory().await;
    insert_user(&pool, USER_A.as_slice(), "alice.one").await;
    insert_group(&pool, GROUP_G.as_slice(), USER_A.as_slice(), "g").await;

    let both = insert_theme(
        &pool,
        THEME_1.as_slice(),
        Some(USER_A.as_slice()),
        Some(GROUP_G.as_slice()),
        USER_A.as_slice(),
        "both",
    )
    .await;
    assert!(
        both.is_err(),
        "a theme cannot be owned by a user AND a group"
    );

    let neither = insert_theme(
        &pool,
        THEME_2.as_slice(),
        None,
        None,
        USER_A.as_slice(),
        "neither",
    )
    .await;
    assert!(neither.is_err(), "a theme must have exactly one owner");
}

#[tokio::test]
async fn theme_names_are_unique_within_a_scope_but_not_across_scopes() {
    let pool = open_in_memory().await;
    insert_user(&pool, USER_A.as_slice(), "alice.one").await;
    insert_user(&pool, USER_B.as_slice(), "bob.two").await;
    insert_group(&pool, GROUP_G.as_slice(), USER_A.as_slice(), "g").await;

    insert_theme(
        &pool,
        THEME_1.as_slice(),
        Some(USER_A.as_slice()),
        None,
        USER_A.as_slice(),
        "dusk",
    )
    .await
    .expect("first personal theme inserts");

    // Same name, same personal scope → rejected.
    let dup = insert_theme(
        &pool,
        THEME_2.as_slice(),
        Some(USER_A.as_slice()),
        None,
        USER_A.as_slice(),
        "dusk",
    )
    .await;
    assert!(dup.is_err(), "a user cannot have two themes named 'dusk'");

    // Same name in a *different* scope (another user, and a group) → allowed.
    insert_theme(
        &pool,
        THEME_2.as_slice(),
        Some(USER_B.as_slice()),
        None,
        USER_B.as_slice(),
        "dusk",
    )
    .await
    .expect("another user may reuse the name");
    insert_theme(
        &pool,
        [13; 16].as_slice(),
        None,
        Some(GROUP_G.as_slice()),
        USER_A.as_slice(),
        "dusk",
    )
    .await
    .expect("a group may reuse a name a member uses personally");
}

#[tokio::test]
async fn personal_themes_are_per_user() {
    let pool = open_in_memory().await;
    insert_user(&pool, USER_A.as_slice(), "alice.one").await;
    insert_user(&pool, USER_B.as_slice(), "bob.two").await;

    insert_theme(
        &pool,
        THEME_1.as_slice(),
        Some(USER_A.as_slice()),
        None,
        USER_A.as_slice(),
        "a-theme",
    )
    .await
    .expect("insert A's theme");

    assert_eq!(
        personal_theme_names(&pool, USER_A.as_slice()).await,
        vec!["a-theme".to_owned()],
        "user A sees their own theme",
    );
    assert!(
        personal_theme_names(&pool, USER_B.as_slice())
            .await
            .is_empty(),
        "user B sees none of user A's personal themes",
    );
}

#[tokio::test]
async fn group_themes_belong_to_their_group() {
    let pool = open_in_memory().await;
    insert_user(&pool, USER_A.as_slice(), "alice.one").await;
    insert_group(&pool, GROUP_G.as_slice(), USER_A.as_slice(), "g").await;
    insert_membership(&pool, GROUP_G.as_slice(), USER_A.as_slice(), "owner").await;

    insert_theme(
        &pool,
        THEME_1.as_slice(),
        None,
        Some(GROUP_G.as_slice()),
        USER_A.as_slice(),
        "shared",
    )
    .await
    .expect("insert group theme");

    assert_eq!(
        group_theme_names(&pool, GROUP_G.as_slice()).await,
        vec!["shared".to_owned()],
        "the group lists its theme",
    );
    assert!(
        personal_theme_names(&pool, USER_A.as_slice())
            .await
            .is_empty(),
        "a group theme is not a personal theme of its creator",
    );
}

#[tokio::test]
async fn deleting_a_user_cascades_their_personal_themes() {
    let pool = open_in_memory().await;
    insert_user(&pool, USER_A.as_slice(), "alice.one").await;
    insert_user(&pool, USER_B.as_slice(), "bob.two").await;
    insert_theme(
        &pool,
        THEME_1.as_slice(),
        Some(USER_A.as_slice()),
        None,
        USER_A.as_slice(),
        "a-theme",
    )
    .await
    .expect("insert A's theme");
    insert_theme(
        &pool,
        THEME_2.as_slice(),
        Some(USER_B.as_slice()),
        None,
        USER_B.as_slice(),
        "b-theme",
    )
    .await
    .expect("insert B's theme");

    sqlx::query("DELETE FROM users WHERE user_id = ?1")
        .bind(USER_A.as_slice())
        .execute(&pool)
        .await
        .expect("delete user A");

    assert!(
        personal_theme_names(&pool, USER_A.as_slice())
            .await
            .is_empty(),
        "user A's personal themes are cascaded away",
    );
    assert_eq!(
        personal_theme_names(&pool, USER_B.as_slice()).await,
        vec!["b-theme".to_owned()],
        "user B's themes are untouched",
    );
}

#[tokio::test]
async fn deleting_a_group_cascades_its_themes() {
    let pool = open_in_memory().await;
    insert_user(&pool, USER_A.as_slice(), "alice.one").await;
    insert_group(&pool, GROUP_G.as_slice(), USER_A.as_slice(), "g").await;
    insert_membership(&pool, GROUP_G.as_slice(), USER_A.as_slice(), "owner").await;
    insert_theme(
        &pool,
        THEME_1.as_slice(),
        None,
        Some(GROUP_G.as_slice()),
        USER_A.as_slice(),
        "shared",
    )
    .await
    .expect("insert group theme");

    sqlx::query("DELETE FROM \"groups\" WHERE group_id = ?1")
        .bind(GROUP_G.as_slice())
        .execute(&pool)
        .await
        .expect("delete group");

    assert!(
        group_theme_names(&pool, GROUP_G.as_slice())
            .await
            .is_empty(),
        "the group's themes are cascaded away",
    );
}

#[tokio::test]
async fn deleting_creator_keeps_group_theme_and_nulls_created_by() {
    let pool = open_in_memory().await;
    // The group is created by A so that deleting B (the theme creator and a
    // group owner) is not blocked by groups.created_by ON DELETE RESTRICT.
    insert_user(&pool, USER_A.as_slice(), "alice.one").await;
    insert_user(&pool, USER_B.as_slice(), "bob.two").await;
    insert_group(&pool, GROUP_G.as_slice(), USER_A.as_slice(), "g").await;
    insert_membership(&pool, GROUP_G.as_slice(), USER_A.as_slice(), "owner").await;
    insert_membership(&pool, GROUP_G.as_slice(), USER_B.as_slice(), "owner").await;
    insert_theme(
        &pool,
        THEME_1.as_slice(),
        None,
        Some(GROUP_G.as_slice()),
        USER_B.as_slice(),
        "shared",
    )
    .await
    .expect("insert group theme created by B");

    sqlx::query("DELETE FROM users WHERE user_id = ?1")
        .bind(USER_B.as_slice())
        .execute(&pool)
        .await
        .expect("delete user B");

    // The group theme survives, but its created_by link is severed.
    assert_eq!(
        group_theme_names(&pool, GROUP_G.as_slice()).await,
        vec!["shared".to_owned()],
        "the group theme outlives its creator's account",
    );
    let created_by: Option<Vec<u8>> =
        sqlx::query_scalar("SELECT created_by FROM themes WHERE theme_id = ?1")
            .bind(THEME_1.as_slice())
            .fetch_one(&pool)
            .await
            .expect("fetch created_by");
    assert!(
        created_by.is_none(),
        "created_by is nulled by ON DELETE SET NULL",
    );
}
