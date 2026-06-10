//! HTTP handlers for saved GLW event data.
//!
//! The render worker is the primary writer here — every successful
//! GLW-overlay render auto-inserts a `saved_glw_data` row through the
//! shared [`insert_glw_data_row`] helper. The CRUD routes below let
//! users list / inspect / rename / delete rows manually.

use std::fmt::Write as _;

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode as ReqwestStatusCode;
use axum::response::{IntoResponse as _, Response};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::auth::{CurrentUser, uuid_from_bytes};
use crate::error::{self, Error};
use crate::library::{self, Destination, GlwDataRow, GlwDataSourceKind, GlwDataView};
use crate::state::AppState;

/// Query parameters for `GET /api/glw`. Both filters are optional and
/// independently honoured; passing neither lists all rows the caller
/// can see in `scope`.
#[derive(Debug, Deserialize)]
pub struct ListQuery {
    /// `"personal"` or `"group:<uuid>"`.
    pub scope: String,
    /// Restrict to rows whose `source_event_id` matches this value.
    #[serde(default)]
    pub event_id: Option<u32>,
    /// Restrict to rows whose `source_event_key` matches this value.
    #[serde(default)]
    pub event_key: Option<String>,
}

/// Response shape for `GET /api/glw`.
#[derive(Debug, Serialize)]
pub struct ListGlwDataResponse {
    /// matching rows, newest fetched_at first.
    pub glw_data: Vec<GlwDataView>,
}

/// Response shape for a single GLW row.
#[expect(
    clippy::module_name_repetitions,
    reason = "matches the workspace convention: <Entity>Response in the <entity> route module"
)]
#[derive(Debug, Serialize)]
pub struct GlwDataResponse {
    /// the requested row.
    pub glw_data: GlwDataView,
}

/// `GET /api/glw?scope=…&event_id=…&event_key=…` — list saved GLW
/// rows in a scope, optionally filtered by source id or key.
///
/// # Errors
///
/// Returns [`Error::BadRequest`] for an invalid scope; [`Error::Forbidden`]
/// if the user is not allowed to view the scope.
pub async fn list(
    user: CurrentUser,
    State(state): State<AppState>,
    Query(query): Query<ListQuery>,
) -> Result<Json<ListGlwDataResponse>, Error> {
    let destination = Destination::parse(&query.scope)?;
    library::assert_can_view(&state.db, user.user_id, destination).await?;
    let rows = fetch_glw_data_for(
        &state,
        user.user_id,
        destination,
        query.event_id,
        query.event_key.as_deref(),
    )
    .await?;
    Ok(Json(ListGlwDataResponse { glw_data: rows }))
}

/// `GET /api/glw/{id}` — fetch a single row's metadata (payload is
/// returned by `GET /api/glw/{id}/payload` — currently inlined but a
/// separate endpoint exists in case the payload grows large enough
/// that we want to keep the list lean).
///
/// # Errors
///
/// Returns [`Error::NotFound`] if the row doesn't exist or is invisible.
pub async fn get(
    user: CurrentUser,
    State(state): State<AppState>,
    Path(glw_data_id): Path<Uuid>,
) -> Result<Json<GlwDataResponse>, Error> {
    let row = library::assert_can_read_glw_data(&state.db, user.user_id, glw_data_id).await?;
    let destination =
        library::destination_from_columns(row.owner_user_id.clone(), row.owner_group_id.clone())?;
    let (created_by_username, created_by_legacy_name) = match row.created_by {
        Some(id) => match lookup_user_names(&state, id).await {
            Ok((u, l)) => (Some(u), Some(l)),
            Err(_) => (None, None),
        },
        None => (None, None),
    };
    let view = build_view(
        &row,
        destination,
        created_by_username,
        created_by_legacy_name,
    );
    Ok(Json(GlwDataResponse { glw_data: view }))
}

/// Body for `PATCH /api/glw/{id}` — currently only `name` can be
/// updated. Source / payload are immutable so that any render
/// pointing at the row by `glw_data_id` still refers to the same
/// underlying GLW event.
#[derive(Debug, Deserialize)]
pub struct RenameRequest {
    /// new display name.
    pub name: String,
}

/// `PATCH /api/glw/{id}` — rename a saved GLW row. Personal owner or
/// group owner only.
///
/// # Errors
///
/// Returns [`Error::Forbidden`] / [`Error::NotFound`] / [`Error::BadRequest`].
pub async fn rename(
    user: CurrentUser,
    State(state): State<AppState>,
    Path(glw_data_id): Path<Uuid>,
    Json(body): Json<RenameRequest>,
) -> Result<Json<GlwDataResponse>, Error> {
    let trimmed = library::sanitise_display_name(&body.name, "name")?;
    let row = library::assert_can_delete_glw_data(&state.db, user.user_id, glw_data_id).await?;
    sqlx::query("UPDATE saved_glw_data SET name = ?1 WHERE glw_data_id = ?2")
        .bind(&trimmed)
        .bind(glw_data_id.as_bytes().to_vec())
        .execute(&state.db)
        .await
        .map_err(|err| {
            tracing::error!("glw data rename failed: {err}");
            Error::Database
        })?;
    let destination =
        library::destination_from_columns(row.owner_user_id.clone(), row.owner_group_id.clone())?;
    let (created_by_username, created_by_legacy_name) = match row.created_by {
        Some(id) => match lookup_user_names(&state, id).await {
            Ok((u, l)) => (Some(u), Some(l)),
            Err(_) => (None, None),
        },
        None => (None, None),
    };
    // Build the view from the row plus the new name so the response
    // reflects what just landed in the DB without a redundant SELECT.
    let mut row_with_new_name = row;
    row_with_new_name.name = trimmed;
    let view = build_view(
        &row_with_new_name,
        destination,
        created_by_username,
        created_by_legacy_name,
    );
    Ok(Json(GlwDataResponse { glw_data: view }))
}

/// `DELETE /api/glw/{id}` — delete a saved GLW row. Personal owner or
/// group owner only. The FK `saved_renders.glw_data_id` is `ON
/// DELETE RESTRICT`, so a row referenced by any render fails with a
/// human-friendly error pointing the user at the dependent renders.
///
/// # Errors
///
/// Returns [`Error::Forbidden`] / [`Error::NotFound`] / [`Error::BadRequest`].
pub async fn delete(
    user: CurrentUser,
    State(state): State<AppState>,
    Path(glw_data_id): Path<Uuid>,
) -> Result<Response, Error> {
    library::assert_can_delete_glw_data(&state.db, user.user_id, glw_data_id).await?;
    let result = sqlx::query("DELETE FROM saved_glw_data WHERE glw_data_id = ?1")
        .bind(glw_data_id.as_bytes().to_vec())
        .execute(&state.db)
        .await;
    match result {
        Ok(_) => Ok((ReqwestStatusCode::NO_CONTENT, "").into_response()),
        Err(err) => {
            if error::is_fk_violation(&err) {
                return Err(Error::BadRequest(
                    "cannot delete this GLW data: one or more saved renders still reference it. \
                     Delete those renders first."
                        .to_owned(),
                ));
            }
            tracing::error!("glw data delete failed: {err}");
            Err(Error::Database)
        }
    }
}

/// Fields needed to persist a fresh `saved_glw_data` row. Shared by
/// the render worker (via `insert_glw_data_row` below) and any future
/// stand-alone POST handler.
#[derive(Debug)]
pub struct InsertGlwData<'a> {
    /// destination scope (personal or a group).
    pub destination: Destination,
    /// the avatar saving the row.
    pub created_by: Uuid,
    /// the display name (caller-supplied or default).
    pub name: &'a str,
    /// where the event came from.
    pub source_kind: GlwDataSourceKind,
    /// originating numeric event id (only meaningful for `EventId`).
    pub source_event_id: Option<u32>,
    /// originating string event key (only meaningful for `EventKey`).
    pub source_event_key: Option<&'a str>,
    /// canonical JSON for the resolved event.
    pub payload_json: &'a str,
    /// numeric event id from the resolved JSON.
    pub event_id: Option<u32>,
    /// string event key from the resolved JSON.
    pub event_key: Option<&'a str>,
    /// human-readable event name from the resolved JSON.
    pub event_name: Option<&'a str>,
    /// when the event was originally fetched / pasted.
    pub fetched_at: DateTime<Utc>,
}

/// Persist a fresh `saved_glw_data` row and return its id. Callers
/// must already have permission to write to `destination`.
///
/// # Errors
///
/// Returns [`Error::Database`] on any underlying SQLite failure.
pub async fn insert_glw_data_row(
    state: &AppState,
    insert: &InsertGlwData<'_>,
) -> Result<Uuid, Error> {
    let glw_data_id = Uuid::new_v4();
    let now = Utc::now();
    let (owner_user, owner_group) = match insert.destination {
        Destination::Personal => (Some(insert.created_by.as_bytes().to_vec()), None),
        Destination::Group { group_id } => (None, Some(group_id.as_bytes().to_vec())),
    };
    sqlx::query(
        "INSERT INTO saved_glw_data \
            (glw_data_id, owner_user_id, owner_group_id, created_by, name, \
             source_kind, source_event_id, source_event_key, payload_json, \
             event_id, event_key, event_name, fetched_at, created_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
    )
    .bind(glw_data_id.as_bytes().to_vec())
    .bind(owner_user)
    .bind(owner_group)
    .bind(insert.created_by.as_bytes().to_vec())
    .bind(insert.name)
    .bind(insert.source_kind.as_db_str())
    .bind(insert.source_event_id.map(i64::from))
    .bind(insert.source_event_key)
    .bind(insert.payload_json)
    .bind(insert.event_id.map(i64::from))
    .bind(insert.event_key)
    .bind(insert.event_name)
    .bind(insert.fetched_at)
    .bind(now)
    .execute(&state.db)
    .await
    .map_err(|err| {
        tracing::error!("insert saved_glw_data failed: {err}");
        Error::Database
    })?;
    Ok(glw_data_id)
}

/// Row shape returned by the listing query (uses a `FromRow` struct
/// because the column count is at sqlx's tuple-arity limit).
#[derive(sqlx::FromRow)]
struct GlwDataListRow {
    /// raw bytes of `saved_glw_data.glw_data_id`.
    glw_data_id: Vec<u8>,
    /// raw bytes of `saved_glw_data.owner_user_id`, if set.
    owner_user_id: Option<Vec<u8>>,
    /// raw bytes of `saved_glw_data.owner_group_id`, if set.
    owner_group_id: Option<Vec<u8>>,
    /// raw bytes of the creating user's id. `None` only if the
    /// account has been deleted (kept here for forward-compatibility
    /// — the column is currently NOT NULL).
    created_by: Option<Vec<u8>>,
    /// the creating user's `username`.
    created_by_username: Option<String>,
    /// the creating user's `legacy_name`.
    created_by_legacy_name: Option<String>,
    /// display name.
    name: String,
    /// the raw `source_kind` text column.
    source_kind: String,
    /// originating numeric event id, when applicable.
    source_event_id: Option<i64>,
    /// originating string event key, when applicable.
    source_event_key: Option<String>,
    /// numeric event id from the resolved JSON.
    event_id: Option<i64>,
    /// string event key from the resolved JSON.
    event_key: Option<String>,
    /// human-readable event name from the resolved JSON.
    event_name: Option<String>,
    /// when the event was fetched / pasted.
    fetched_at: DateTime<Utc>,
    /// row creation timestamp.
    created_at: DateTime<Utc>,
}

/// Run the list query for the given scope and optional source filters.
async fn fetch_glw_data_for(
    state: &AppState,
    current_user: Uuid,
    destination: Destination,
    filter_event_id: Option<u32>,
    filter_event_key: Option<&str>,
) -> Result<Vec<GlwDataView>, Error> {
    let mut sql = String::from(
        "SELECT g.glw_data_id, g.owner_user_id, g.owner_group_id, g.created_by, \
                u.username AS created_by_username, u.legacy_name AS created_by_legacy_name, \
                g.name, g.source_kind, g.source_event_id, g.source_event_key, \
                g.event_id, g.event_key, g.event_name, g.fetched_at, g.created_at \
         FROM saved_glw_data AS g \
         LEFT JOIN users AS u ON u.user_id = g.created_by ",
    );
    match destination {
        Destination::Personal => {
            sql.push_str("WHERE g.owner_user_id = ?1 ");
        }
        Destination::Group { .. } => {
            // The JOIN against group_memberships enforces visibility at
            // the SQL layer, matching the saved_notecards list query.
            sql.push_str(
                "JOIN group_memberships AS gm \
                   ON gm.group_id = g.owner_group_id AND gm.user_id = ?2 \
                 WHERE g.owner_group_id = ?1 ",
            );
        }
    }
    if filter_event_id.is_some() {
        sql.push_str("AND g.source_event_id = ?3 ");
    }
    if filter_event_key.is_some() {
        // Bind index depends on whether event_id is also bound; build
        // a fresh placeholder index up front.
        let idx = 3_u8.saturating_add(u8::from(filter_event_id.is_some()));
        // Writing directly into the SQL buffer avoids an intermediate
        // allocation that the `clippy::format-push-string` lint flags.
        // `String::write_fmt` is infallible; `unwrap_or(())` discards
        // the unreachable `Err` arm without tripping the banned-method
        // lints (`unwrap()` / `expect()` / `panic!`).
        write!(sql, "AND g.source_event_key = ?{idx} ").unwrap_or(());
    }
    sql.push_str("ORDER BY g.fetched_at DESC");

    let mut query = sqlx::query_as::<_, GlwDataListRow>(&sql);
    match destination {
        Destination::Personal => {
            query = query.bind(current_user.as_bytes().to_vec());
        }
        Destination::Group { group_id } => {
            query = query
                .bind(group_id.as_bytes().to_vec())
                .bind(current_user.as_bytes().to_vec());
        }
    }
    if let Some(id) = filter_event_id {
        query = query.bind(i64::from(id));
    }
    if let Some(key) = filter_event_key {
        query = query.bind(key);
    }
    let rows: Vec<GlwDataListRow> = query.fetch_all(&state.db).await.map_err(|err| {
        tracing::error!("list saved_glw_data failed: {err}");
        Error::Database
    })?;

    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        let glw_data_id = uuid_from_bytes(&row.glw_data_id).ok_or_else(|| {
            tracing::error!("bad glw_data uuid");
            Error::Database
        })?;
        let row_dest = library::destination_from_columns(row.owner_user_id, row.owner_group_id)?;
        let created_by = row
            .created_by
            .as_deref()
            .map(uuid_from_bytes)
            .map(|opt| {
                opt.ok_or_else(|| {
                    tracing::error!("bad created_by uuid in saved_glw_data");
                    Error::Database
                })
            })
            .transpose()?;
        let source_kind = GlwDataSourceKind::from_db_str(&row.source_kind).ok_or_else(|| {
            tracing::error!(
                "unrecognised source_kind `{}` in saved_glw_data",
                row.source_kind
            );
            Error::Database
        })?;
        out.push(GlwDataView {
            glw_data_id,
            destination: row_dest,
            created_by,
            created_by_username: row.created_by_username,
            created_by_legacy_name: row.created_by_legacy_name,
            name: row.name,
            source_kind,
            source_event_id: row.source_event_id.and_then(|v| u32::try_from(v).ok()),
            source_event_key: row.source_event_key,
            event_id: row.event_id.and_then(|v| u32::try_from(v).ok()),
            event_key: row.event_key,
            event_name: row.event_name,
            fetched_at: row.fetched_at,
            created_at: row.created_at,
        });
    }
    Ok(out)
}

/// Build a [`GlwDataView`] from a fetched row and the resolved creator
/// display name pair.
fn build_view(
    row: &GlwDataRow,
    destination: Destination,
    created_by_username: Option<String>,
    created_by_legacy_name: Option<String>,
) -> GlwDataView {
    GlwDataView {
        glw_data_id: row.glw_data_id,
        destination,
        created_by: row.created_by,
        created_by_username,
        created_by_legacy_name,
        name: row.name.clone(),
        source_kind: row.source_kind,
        source_event_id: row.source_event_id,
        source_event_key: row.source_event_key.clone(),
        event_id: row.event_id,
        event_key: row.event_key.clone(),
        event_name: row.event_name.clone(),
        fetched_at: row.fetched_at,
        created_at: row.created_at,
    }
}

/// Look up a user's display fields for view-building. Mirrors the
/// helper in [`crate::routes::notecards`].
async fn lookup_user_names(state: &AppState, user_id: Uuid) -> Result<(String, String), Error> {
    let row: Option<(String, String)> =
        sqlx::query_as("SELECT username, legacy_name FROM users WHERE user_id = ?1")
            .bind(user_id.as_bytes().to_vec())
            .fetch_optional(&state.db)
            .await
            .map_err(|err| {
                tracing::error!("user name lookup failed: {err}");
                Error::Database
            })?;
    row.ok_or_else(|| Error::NotFound(format!("user {user_id}")))
}
