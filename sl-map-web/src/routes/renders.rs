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
use crate::jobs::Metadata;
use crate::library::{self, Destination, RenderView};
use crate::routes::render::SavedRenderSettings;
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
    /// the requested render's metadata (status, owner, settings,
    /// timestamps) — the image bytes are fetched separately via the
    /// `/image` and `/image-without-route` sub-routes.
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
    let creator_names = match row.created_by {
        Some(id) => lookup_user_names(&state, id).await.ok(),
        None => None,
    };
    let (creator_username, creator_legacy) = match creator_names {
        Some((u, l)) => (Some(u), Some(l)),
        None => (None, None),
    };
    let notecard_name = match row.notecard_id {
        Some(id) => lookup_notecard_name(&state, id).await?,
        None => None,
    };
    let glw_data_name = match row.glw_data_id {
        Some(id) => lookup_glw_data_name(&state, id).await?,
        None => None,
    };
    let view = RenderView {
        render_id,
        destination,
        created_by: row.created_by,
        created_by_username: creator_username,
        created_by_legacy_name: creator_legacy,
        notecard_id: row.notecard_id,
        notecard_name,
        kind: row.kind,
        status: row.status,
        error_message: row.error_message,
        created_at: row.created_at,
        finished_at: row.finished_at,
        has_without_route: row.image_without_route_filename.is_some(),
        content_type: row.content_type,
        lower_left_x: row.lower_left_x,
        lower_left_y: row.lower_left_y,
        upper_right_x: row.upper_right_x,
        upper_right_y: row.upper_right_y,
        glw_data_id: row.glw_data_id,
        glw_data_name,
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

/// `GET /api/renders/{id}/download-without-route` — like
/// `/image-without-route` but with `Content-Disposition: attachment`.
///
/// # Errors
///
/// As [`image()`].
pub async fn download_without_route(
    user: CurrentUser,
    State(state): State<AppState>,
    Path(render_id): Path<Uuid>,
) -> Result<Response, Error> {
    serve_image(user, state, render_id, true, true).await
}

/// `GET /api/renders/{id}/metadata` — serve the metadata JSON.
///
/// The stored bytes are round-tripped through [`Metadata`] rather than
/// being replayed verbatim, so an unexpected writer to the column cannot
/// cause the API to serve arbitrary trusted-content-type JSON.
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
    let raw = row
        .metadata_json
        .ok_or_else(|| Error::NotFound(format!("render {render_id} has no metadata yet")))?;
    let parsed: Metadata = serde_json::from_str(&raw)?;
    Ok(Json(parsed).into_response())
}

/// `GET /api/renders/{id}/settings` — serve the settings JSON used to launch
/// the render. Used by the UI's "Regenerate" button.
///
/// The stored bytes are round-tripped through [`SavedRenderSettings`]
/// rather than being replayed verbatim; same rationale as [`metadata`].
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
    let parsed: SavedRenderSettings = serde_json::from_str(&row.settings_json)?;
    Ok(Json(parsed).into_response())
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
    // Compute the download stem before we move fields out of `row` for
    // the image lookup below.
    let download_stem = if attachment {
        Some(render_download_stem(&state, render_id, &row).await)
    } else {
        None
    };
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
    if let Some(stem) = download_stem {
        let ext = storage::ext_for_content_type(&content_type);
        let suffix = if want_without_route { "-no-route" } else { "" };
        let disposition = format!("attachment; filename=\"{stem}{suffix}.{ext}\"");
        if let Ok(v) = axum::http::HeaderValue::from_str(&disposition) {
            drop(headers.insert(header::CONTENT_DISPOSITION, v));
        }
    }
    Ok((headers, bytes).into_response())
}

/// Compose a download filename stem for a saved render. Prefers, in
/// order: the linked notecard's display name (sanitised), the grid
/// rectangle as `grid-{ll_x}-{ll_y}-{ur_x}-{ur_y}` when the bounds are
/// known, and `render-{render_id}` as a last-resort fallback. The stem
/// has no extension; the caller appends the right one for the content
/// type.
async fn render_download_stem(
    state: &AppState,
    render_id: Uuid,
    row: &library::RenderRow,
) -> String {
    if let Some(id) = row.notecard_id
        && let Ok(Some(name)) = lookup_notecard_name(state, id).await
        && let Some(stem) = crate::routes::notecards::sanitise_for_filename(&name)
    {
        return stem;
    }
    if let (Some(ll_x), Some(ll_y), Some(ur_x), Some(ur_y)) = (
        row.lower_left_x,
        row.lower_left_y,
        row.upper_right_x,
        row.upper_right_y,
    ) {
        return format!("grid-{ll_x}-{ll_y}-{ur_x}-{ur_y}");
    }
    format!("render-{render_id}")
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

/// Row shape returned by the listing queries. Uses a `FromRow` struct
/// instead of a tuple because the column list is past sqlx's tuple-
/// `FromRow` arity (16).
#[derive(sqlx::FromRow)]
struct RenderListRow {
    /// raw bytes of `saved_renders.render_id`.
    render_id: Vec<u8>,
    /// raw bytes of `saved_renders.owner_user_id`, if set.
    owner_user_id: Option<Vec<u8>>,
    /// raw bytes of `saved_renders.owner_group_id`, if set.
    owner_group_id: Option<Vec<u8>>,
    /// raw bytes of the user that created the render. `None` when the
    /// account has been deleted (FK is `ON DELETE SET NULL`).
    created_by: Option<Vec<u8>>,
    /// the creating user's `username`, if the account still exists.
    creator_username: Option<String>,
    /// the creating user's `legacy_name`, if the account still exists.
    creator_legacy_name: Option<String>,
    /// raw bytes of the linked notecard id, if any.
    notecard_id: Option<Vec<u8>>,
    /// the linked notecard's display name, if any.
    notecard_name: Option<String>,
    /// render kind (`grid_rectangle` or `usb_notecard`).
    kind: String,
    /// render status (`in_progress`, `done`, `failed`).
    status: String,
    /// error message if `status = 'failed'`.
    error_message: Option<String>,
    /// filename of the without-route image, if one was saved.
    image_without_route_filename: Option<String>,
    /// MIME type of the stored image.
    content_type: Option<String>,
    /// row creation timestamp.
    created_at: DateTime<Utc>,
    /// terminal-state timestamp, if any.
    finished_at: Option<DateTime<Utc>>,
    /// lower-left x grid coordinate of the rendered rectangle, if known.
    lower_left_x: Option<i64>,
    /// lower-left y grid coordinate of the rendered rectangle, if known.
    lower_left_y: Option<i64>,
    /// upper-right x grid coordinate of the rendered rectangle, if known.
    upper_right_x: Option<i64>,
    /// upper-right y grid coordinate of the rendered rectangle, if known.
    upper_right_y: Option<i64>,
    /// raw bytes of the linked saved_glw_data row, if any.
    glw_data_id: Option<Vec<u8>>,
    /// display name of the linked saved_glw_data row, if any.
    glw_data_name: Option<String>,
}

/// Conversion helper from a raw list-query row to a `RenderView`. Defined
/// as a trait so we can move the bulky destructuring out of the main loop
/// without committing to a free function name in the module index.
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
        let render_id = uuid_from_bytes(&self.render_id).ok_or_else(|| {
            tracing::error!("bad render uuid");
            Error::Database
        })?;
        let destination =
            library::destination_from_columns(self.owner_user_id, self.owner_group_id)?;
        let created_by = self
            .created_by
            .as_deref()
            .map(uuid_from_bytes)
            .map(|opt| {
                opt.ok_or_else(|| {
                    tracing::error!("bad created_by uuid");
                    Error::Database
                })
            })
            .transpose()?;
        let notecard_id = self
            .notecard_id
            .as_deref()
            .map(uuid_from_bytes)
            .map(|opt| {
                opt.ok_or_else(|| {
                    tracing::error!("bad notecard uuid");
                    Error::Database
                })
            })
            .transpose()?;
        let glw_data_id = self
            .glw_data_id
            .as_deref()
            .map(uuid_from_bytes)
            .map(|opt| {
                opt.ok_or_else(|| {
                    tracing::error!("bad glw_data uuid");
                    Error::Database
                })
            })
            .transpose()?;
        Ok(RenderView {
            render_id,
            destination,
            created_by,
            created_by_username: self.creator_username,
            created_by_legacy_name: self.creator_legacy_name,
            notecard_id,
            notecard_name: self.notecard_name,
            kind: self.kind,
            status: self.status,
            error_message: self.error_message,
            created_at: self.created_at,
            finished_at: self.finished_at,
            has_without_route: self.image_without_route_filename.is_some(),
            content_type: self.content_type,
            lower_left_x: self.lower_left_x.and_then(|v| u16::try_from(v).ok()),
            lower_left_y: self.lower_left_y.and_then(|v| u16::try_from(v).ok()),
            upper_right_x: self.upper_right_x.and_then(|v| u16::try_from(v).ok()),
            upper_right_y: self.upper_right_y.and_then(|v| u16::try_from(v).ok()),
            glw_data_id,
            glw_data_name: self.glw_data_name,
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

/// Look up a saved notecard's display name. Returns `None` if the row
/// has been deleted out from under us (which the `ON DELETE RESTRICT` FK
/// on `saved_renders.notecard_id` should prevent in normal operation).
async fn lookup_notecard_name(
    state: &AppState,
    notecard_id: Uuid,
) -> Result<Option<String>, Error> {
    let row: Option<(String,)> =
        sqlx::query_as("SELECT name FROM saved_notecards WHERE notecard_id = ?1")
            .bind(notecard_id.as_bytes().to_vec())
            .fetch_optional(&state.db)
            .await
            .map_err(|err| {
                tracing::error!("notecard name lookup failed: {err}");
                Error::Database
            })?;
    Ok(row.map(|(name,)| name))
}

/// Look up a saved GLW data row's display name. Returns `None` if
/// the row has been deleted out from under us (which the
/// `ON DELETE RESTRICT` FK should prevent in normal operation).
async fn lookup_glw_data_name(
    state: &AppState,
    glw_data_id: Uuid,
) -> Result<Option<String>, Error> {
    let row: Option<(String,)> =
        sqlx::query_as("SELECT name FROM saved_glw_data WHERE glw_data_id = ?1")
            .bind(glw_data_id.as_bytes().to_vec())
            .fetch_optional(&state.db)
            .await
            .map_err(|err| {
                tracing::error!("glw data name lookup failed: {err}");
                Error::Database
            })?;
    Ok(row.map(|(name,)| name))
}

/// SQL: list a user's personal renders.
const LIST_PERSONAL_SQL: &str = "SELECT r.render_id, r.owner_user_id, r.owner_group_id, \
        r.created_by, u.username AS creator_username, u.legacy_name AS creator_legacy_name, r.notecard_id, n.name AS notecard_name, r.kind, r.status, \
        r.error_message, r.image_without_route_filename, r.content_type, \
        r.created_at, r.finished_at, \
        r.lower_left_x, r.lower_left_y, r.upper_right_x, r.upper_right_y, \
        r.glw_data_id, g.name AS glw_data_name \
     FROM saved_renders AS r \
     LEFT JOIN users AS u ON u.user_id = r.created_by \
     LEFT JOIN saved_notecards AS n ON n.notecard_id = r.notecard_id \
     LEFT JOIN saved_glw_data AS g ON g.glw_data_id = r.glw_data_id \
     WHERE r.owner_user_id = ?1 \
     ORDER BY r.created_at DESC";

/// SQL: list a user's personal renders filtered by status.
const LIST_PERSONAL_STATUS_SQL: &str = "SELECT r.render_id, r.owner_user_id, r.owner_group_id, \
        r.created_by, u.username AS creator_username, u.legacy_name AS creator_legacy_name, r.notecard_id, n.name AS notecard_name, r.kind, r.status, \
        r.error_message, r.image_without_route_filename, r.content_type, \
        r.created_at, r.finished_at, \
        r.lower_left_x, r.lower_left_y, r.upper_right_x, r.upper_right_y, \
        r.glw_data_id, g.name AS glw_data_name \
     FROM saved_renders AS r \
     LEFT JOIN users AS u ON u.user_id = r.created_by \
     LEFT JOIN saved_notecards AS n ON n.notecard_id = r.notecard_id \
     LEFT JOIN saved_glw_data AS g ON g.glw_data_id = r.glw_data_id \
     WHERE r.owner_user_id = ?1 AND r.status = ?2 \
     ORDER BY r.created_at DESC";

/// SQL: list a group's renders. The JOIN against `group_memberships` and the
/// `(gm.role = 'owner' OR r.status = 'done')` clause fold both the membership
/// check and the member-only-finished rule into SQL so a forgotten
/// `assert_can_view` call cannot leak rows.
const LIST_GROUP_SQL: &str = "SELECT r.render_id, r.owner_user_id, r.owner_group_id, \
        r.created_by, u.username AS creator_username, u.legacy_name AS creator_legacy_name, r.notecard_id, n.name AS notecard_name, r.kind, r.status, \
        r.error_message, r.image_without_route_filename, r.content_type, \
        r.created_at, r.finished_at, \
        r.lower_left_x, r.lower_left_y, r.upper_right_x, r.upper_right_y, \
        r.glw_data_id, g.name AS glw_data_name \
     FROM saved_renders AS r \
     LEFT JOIN users AS u ON u.user_id = r.created_by \
     JOIN group_memberships AS gm \
       ON gm.group_id = r.owner_group_id AND gm.user_id = ?2 \
     LEFT JOIN saved_notecards AS n ON n.notecard_id = r.notecard_id \
     LEFT JOIN saved_glw_data AS g ON g.glw_data_id = r.glw_data_id \
     WHERE r.owner_group_id = ?1 \
       AND (gm.role = 'owner' OR r.status = 'done') \
     ORDER BY r.created_at DESC";

/// SQL: list a group's renders filtered by status. Same membership and
/// visibility folding as `LIST_GROUP_SQL`.
const LIST_GROUP_STATUS_SQL: &str = "SELECT r.render_id, r.owner_user_id, r.owner_group_id, \
        r.created_by, u.username AS creator_username, u.legacy_name AS creator_legacy_name, r.notecard_id, n.name AS notecard_name, r.kind, r.status, \
        r.error_message, r.image_without_route_filename, r.content_type, \
        r.created_at, r.finished_at, \
        r.lower_left_x, r.lower_left_y, r.upper_right_x, r.upper_right_y, \
        r.glw_data_id, g.name AS glw_data_name \
     FROM saved_renders AS r \
     LEFT JOIN users AS u ON u.user_id = r.created_by \
     JOIN group_memberships AS gm \
       ON gm.group_id = r.owner_group_id AND gm.user_id = ?2 \
     LEFT JOIN saved_notecards AS n ON n.notecard_id = r.notecard_id \
     LEFT JOIN saved_glw_data AS g ON g.glw_data_id = r.glw_data_id \
     WHERE r.owner_group_id = ?1 AND r.status = ?3 \
       AND (gm.role = 'owner' OR r.status = 'done') \
     ORDER BY r.created_at DESC";
