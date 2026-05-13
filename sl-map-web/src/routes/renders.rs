//! HTTP handlers for the persisted saved-renders library.
//!
//! These endpoints serve the on-disk image files and metadata for
//! `saved_renders` rows. They are separate from `/api/render/{id}/...`
//! (which serves the in-memory job state for live progress) so that
//! finished renders remain accessible after the in-memory job has been
//! evicted.

use std::sync::atomic::Ordering;

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode as ReqwestStatusCode;
use axum::http::header;
use axum::response::{IntoResponse as _, Response};
use chrono::DateTime;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::auth::{CurrentUser, uuid_from_bytes};
use crate::error::Error;
use crate::library::{self, Destination, RenderView};
use crate::state::AppState;
use crate::storage;

/// Query parameters for `GET /api/renders`.
#[derive(Debug, Deserialize)]
pub struct ListQuery {
    /// `"personal"` or `"group:<uuid>"`.
    pub scope: String,
    /// Optional status filter: `in_progress`, `done`, or `failed`.
    pub status: Option<String>,
}

/// Response carrying a list of saved renders.
#[derive(Debug, Serialize)]
pub struct ListRendersResponse {
    /// the renders, newest first.
    pub renders: Vec<RenderView>,
}

/// Response carrying a single saved render's metadata.
#[derive(Debug, Serialize)]
pub struct RenderResponse {
    /// the render.
    pub render: RenderView,
}

/// `GET /api/renders?scope=...&status=...` — list renders in a scope.
///
/// Group members (non-owners) only see `status='done'` rows.
///
/// # Errors
///
/// Returns [`Error::BadRequest`] for an invalid scope; [`Error::Forbidden`]
/// if the caller is not allowed to view that scope.
pub async fn list(
    user: CurrentUser,
    State(state): State<AppState>,
    Query(query): Query<ListQuery>,
) -> Result<Json<ListRendersResponse>, Error> {
    let destination = Destination::parse(&query.scope)?;
    library::assert_can_view(&state.db, user.user_id, destination).await?;
    // The member-only-finished restriction is folded into the group SQL
    // (`gm.role = 'owner' OR r.status = 'done'`); a member supplying
    // `status=in_progress` simply gets an empty result.
    let status = query
        .status
        .as_ref()
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty());
    let renders = fetch_renders_for(&state, user.user_id, destination, status.as_deref()).await?;
    Ok(Json(ListRendersResponse { renders }))
}

/// `GET /api/renders/{id}` — fetch a single render's metadata.
///
/// # Errors
///
/// Returns [`Error::Forbidden`] / [`Error::NotFound`] as appropriate.
pub async fn get(
    user: CurrentUser,
    State(state): State<AppState>,
    Path(render_id): Path<Uuid>,
) -> Result<Json<RenderResponse>, Error> {
    let row = library::assert_can_read_render(&state.db, user.user_id, render_id).await?;
    let destination =
        library::destination_from_columns(row.owner_user_id.clone(), row.owner_group_id.clone())?;
    let (creator_username, creator_legacy) = lookup_user_names(&state, row.created_by).await?;
    let view = RenderView {
        render_id,
        destination,
        created_by: row.created_by,
        created_by_username: creator_username,
        created_by_legacy_name: creator_legacy,
        notecard_id: row.notecard_id,
        kind: row.kind,
        status: row.status,
        error_message: row.error_message,
        created_at: row.created_at,
        finished_at: row.finished_at,
        has_without_route: row.image_without_route_filename.is_some(),
        content_type: row.content_type,
    };
    Ok(Json(RenderResponse { render: view }))
}

/// `DELETE /api/renders/{id}` — delete a saved render and its image files.
/// Personal owner or group owner only.
///
/// # Errors
///
/// Returns [`Error::Forbidden`] for permission failures.
pub async fn delete(
    user: CurrentUser,
    State(state): State<AppState>,
    Path(render_id): Path<Uuid>,
) -> Result<Response, Error> {
    let row = library::assert_can_delete_render(&state.db, user.user_id, render_id).await?;
    sqlx::query("DELETE FROM saved_renders WHERE render_id = ?1")
        .bind(render_id.as_bytes().to_vec())
        .execute(&state.db)
        .await
        .map_err(|err| {
            tracing::error!("render delete failed: {err}");
            Error::Database
        })?;
    // Best-effort unlink of the image files; raise the sweeper flag if it
    // fails so the orphan sweeper retries.
    for filename in [row.image_filename, row.image_without_route_filename]
        .into_iter()
        .flatten()
    {
        if let Err(err) = storage::try_delete_render_file(&state.config.storage_dir, &filename) {
            tracing::warn!("inline unlink of {filename} failed: {err}");
            state.library_cleanup_dirty.store(true, Ordering::Release);
        }
    }
    Ok((ReqwestStatusCode::NO_CONTENT, "").into_response())
}

/// `GET /api/renders/{id}/image` — serve the persisted primary image.
///
/// # Errors
///
/// As for [`get`]; also [`Error::BadRequest`] if the render is not in `done`
/// state.
pub async fn image(
    user: CurrentUser,
    State(state): State<AppState>,
    Path(render_id): Path<Uuid>,
) -> Result<Response, Error> {
    serve_image(user, state, render_id, false, false).await
}

/// `GET /api/renders/{id}/image-without-route` — serve the without-route
/// variant if one was saved.
///
/// # Errors
///
/// As [`image()`].
pub async fn image_without_route(
    user: CurrentUser,
    State(state): State<AppState>,
    Path(render_id): Path<Uuid>,
) -> Result<Response, Error> {
    serve_image(user, state, render_id, true, false).await
}

/// `GET /api/renders/{id}/download` — like `/image` but with
/// `Content-Disposition: attachment` so the browser saves rather than
/// inline-renders.
///
/// # Errors
///
/// As [`image()`].
pub async fn download(
    user: CurrentUser,
    State(state): State<AppState>,
    Path(render_id): Path<Uuid>,
) -> Result<Response, Error> {
    serve_image(user, state, render_id, false, true).await
}

/// `GET /api/renders/{id}/metadata` — serve the metadata JSON.
///
/// # Errors
///
/// As [`get`].
pub async fn metadata(
    user: CurrentUser,
    State(state): State<AppState>,
    Path(render_id): Path<Uuid>,
) -> Result<Response, Error> {
    let row = library::assert_can_read_render(&state.db, user.user_id, render_id).await?;
    let body = row
        .metadata_json
        .ok_or_else(|| Error::NotFound(format!("render {render_id} has no metadata yet")))?;
    let headers = [(header::CONTENT_TYPE, "application/json; charset=utf-8")];
    Ok((headers, body).into_response())
}

/// `GET /api/renders/{id}/settings` — serve the settings JSON used to launch
/// the render. Used by the UI's "Regenerate" button.
///
/// # Errors
///
/// As [`get`].
pub async fn settings(
    user: CurrentUser,
    State(state): State<AppState>,
    Path(render_id): Path<Uuid>,
) -> Result<Response, Error> {
    let row = library::assert_can_read_render(&state.db, user.user_id, render_id).await?;
    let headers = [(header::CONTENT_TYPE, "application/json; charset=utf-8")];
    Ok((headers, row.settings_json).into_response())
}

/// Shared body for `image`, `image_without_route`, `download`.
async fn serve_image(
    user: CurrentUser,
    state: AppState,
    render_id: Uuid,
    want_without_route: bool,
    attachment: bool,
) -> Result<Response, Error> {
    let row = library::assert_can_read_render(&state.db, user.user_id, render_id).await?;
    if row.status != "done" {
        return Err(Error::BadRequest(format!(
            "render is not finished (status={})",
            row.status
        )));
    }
    let filename = if want_without_route {
        row.image_without_route_filename.ok_or_else(|| {
            Error::NotFound(format!("render {render_id} has no without-route variant"))
        })?
    } else {
        row.image_filename
            .ok_or_else(|| Error::NotFound(format!("render {render_id} has no image")))?
    };
    let content_type = row
        .content_type
        .unwrap_or_else(|| "application/octet-stream".to_owned());
    let bytes = storage::read_render_file(&state.config.storage_dir, &filename).await?;
    let mut headers = axum::http::HeaderMap::new();
    if let Ok(v) = axum::http::HeaderValue::from_str(&content_type) {
        drop(headers.insert(header::CONTENT_TYPE, v));
    }
    if attachment {
        let ext = storage::ext_for_content_type(&content_type);
        let disposition = format!("attachment; filename=\"render-{render_id}.{ext}\"");
        if let Ok(v) = axum::http::HeaderValue::from_str(&disposition) {
            drop(headers.insert(header::CONTENT_DISPOSITION, v));
        }
    }
    Ok((headers, bytes).into_response())
}

/// Run the list query for the given scope. The `status` filter, if Some, is
/// applied with an `=` comparison.
async fn fetch_renders_for(
    state: &AppState,
    current_user: Uuid,
    destination: Destination,
    status: Option<&str>,
) -> Result<Vec<RenderView>, Error> {
    let rows: Vec<RenderListRow> = match (destination, status) {
        (Destination::Personal, None) => {
            sqlx::query_as(LIST_PERSONAL_SQL)
                .bind(current_user.as_bytes().to_vec())
                .fetch_all(&state.db)
                .await
        }
        (Destination::Personal, Some(s)) => {
            sqlx::query_as(LIST_PERSONAL_STATUS_SQL)
                .bind(current_user.as_bytes().to_vec())
                .bind(s)
                .fetch_all(&state.db)
                .await
        }
        (Destination::Group { group_id }, None) => {
            sqlx::query_as(LIST_GROUP_SQL)
                .bind(group_id.as_bytes().to_vec())
                .bind(current_user.as_bytes().to_vec())
                .fetch_all(&state.db)
                .await
        }
        (Destination::Group { group_id }, Some(s)) => {
            sqlx::query_as(LIST_GROUP_STATUS_SQL)
                .bind(group_id.as_bytes().to_vec())
                .bind(current_user.as_bytes().to_vec())
                .bind(s)
                .fetch_all(&state.db)
                .await
        }
    }
    .map_err(|err| {
        tracing::error!("list renders failed: {err}");
        Error::Database
    })?;
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        out.push(row.into_view()?);
    }
    Ok(out)
}

/// Tuple alias for the row shape selected by the listing queries.
type RenderListRow = (
    Vec<u8>,
    Option<Vec<u8>>,
    Option<Vec<u8>>,
    Vec<u8>,
    String,
    String,
    Option<Vec<u8>>,
    String,
    String,
    Option<String>,
    Option<String>,
    Option<String>,
    DateTime<Utc>,
    Option<DateTime<Utc>>,
);

/// Conversion helper from a raw list-query row to a `RenderView`. Defined
/// as a trait so we can move the bulky tuple-destructuring out of the main
/// loop without committing to a free function name in the module index.
trait IntoView {
    /// Convert a raw list-query row into a `RenderView`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Database`] if a UUID blob is malformed.
    fn into_view(self) -> Result<RenderView, Error>;
}

impl IntoView for RenderListRow {
    fn into_view(self) -> Result<RenderView, Error> {
        let (
            rid_bytes,
            owner_user,
            owner_group,
            created_by_bytes,
            creator_username,
            creator_legacy_name,
            notecard_bytes,
            kind,
            status,
            error_message,
            image_without_route_filename,
            content_type,
            created_at,
            finished_at,
        ) = self;
        let render_id = uuid_from_bytes(&rid_bytes).ok_or_else(|| {
            tracing::error!("bad render uuid");
            Error::Database
        })?;
        let destination = library::destination_from_columns(owner_user, owner_group)?;
        let created_by = uuid_from_bytes(&created_by_bytes).ok_or_else(|| {
            tracing::error!("bad created_by uuid");
            Error::Database
        })?;
        let notecard_id = notecard_bytes
            .as_deref()
            .map(uuid_from_bytes)
            .map(|opt| {
                opt.ok_or_else(|| {
                    tracing::error!("bad notecard uuid");
                    Error::Database
                })
            })
            .transpose()?;
        Ok(RenderView {
            render_id,
            destination,
            created_by,
            created_by_username: creator_username,
            created_by_legacy_name: creator_legacy_name,
            notecard_id,
            kind,
            status,
            error_message,
            created_at,
            finished_at,
            has_without_route: image_without_route_filename.is_some(),
            content_type,
        })
    }
}

/// Look up a user's display fields for view-building.
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

/// SQL: list a user's personal renders.
const LIST_PERSONAL_SQL: &str = "SELECT r.render_id, r.owner_user_id, r.owner_group_id, \
        r.created_by, u.username, u.legacy_name, r.notecard_id, r.kind, r.status, \
        r.error_message, r.image_without_route_filename, r.content_type, \
        r.created_at, r.finished_at \
     FROM saved_renders AS r \
     JOIN users AS u ON u.user_id = r.created_by \
     WHERE r.owner_user_id = ?1 \
     ORDER BY r.created_at DESC";

/// SQL: list a user's personal renders filtered by status.
const LIST_PERSONAL_STATUS_SQL: &str = "SELECT r.render_id, r.owner_user_id, r.owner_group_id, \
        r.created_by, u.username, u.legacy_name, r.notecard_id, r.kind, r.status, \
        r.error_message, r.image_without_route_filename, r.content_type, \
        r.created_at, r.finished_at \
     FROM saved_renders AS r \
     JOIN users AS u ON u.user_id = r.created_by \
     WHERE r.owner_user_id = ?1 AND r.status = ?2 \
     ORDER BY r.created_at DESC";

/// SQL: list a group's renders. The JOIN against `group_memberships` and the
/// `(gm.role = 'owner' OR r.status = 'done')` clause fold both the membership
/// check and the member-only-finished rule into SQL so a forgotten
/// `assert_can_view` call cannot leak rows.
const LIST_GROUP_SQL: &str = "SELECT r.render_id, r.owner_user_id, r.owner_group_id, \
        r.created_by, u.username, u.legacy_name, r.notecard_id, r.kind, r.status, \
        r.error_message, r.image_without_route_filename, r.content_type, \
        r.created_at, r.finished_at \
     FROM saved_renders AS r \
     JOIN users AS u ON u.user_id = r.created_by \
     JOIN group_memberships AS gm \
       ON gm.group_id = r.owner_group_id AND gm.user_id = ?2 \
     WHERE r.owner_group_id = ?1 \
       AND (gm.role = 'owner' OR r.status = 'done') \
     ORDER BY r.created_at DESC";

/// SQL: list a group's renders filtered by status. Same membership and
/// visibility folding as `LIST_GROUP_SQL`.
const LIST_GROUP_STATUS_SQL: &str = "SELECT r.render_id, r.owner_user_id, r.owner_group_id, \
        r.created_by, u.username, u.legacy_name, r.notecard_id, r.kind, r.status, \
        r.error_message, r.image_without_route_filename, r.content_type, \
        r.created_at, r.finished_at \
     FROM saved_renders AS r \
     JOIN users AS u ON u.user_id = r.created_by \
     JOIN group_memberships AS gm \
       ON gm.group_id = r.owner_group_id AND gm.user_id = ?2 \
     WHERE r.owner_group_id = ?1 AND r.status = ?3 \
       AND (gm.role = 'owner' OR r.status = 'done') \
     ORDER BY r.created_at DESC";
