//! HTTP handlers for saved notecards.

use axum::Json;
use axum::extract::{Multipart, Path, Query, State};
use axum::http::StatusCode as ReqwestStatusCode;
use axum::http::header;
use axum::response::{IntoResponse as _, Response};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sl_types::map::USBNotecard;
use uuid::Uuid;

use crate::auth::{CurrentUser, uuid_from_bytes};
use crate::error::{self, Error};
use crate::library::{self, Destination, NotecardView};
use crate::rate_limit::{self, RateCategory};
use crate::state::AppState;

/// Query parameters for `GET /api/notecards`.
#[derive(Debug, Deserialize)]
pub struct ScopeQuery {
    /// `"personal"` or `"group:<uuid>"`.
    pub scope: String,
}

/// Response carrying the listing of notecards in a scope.
#[derive(Debug, Serialize)]
pub struct ListNotecardsResponse {
    /// the saved notecards visible in the requested scope, newest first.
    pub notecards: Vec<NotecardView>,
}

/// Response carrying a single saved notecard's metadata.
#[derive(Debug, Serialize)]
pub struct NotecardResponse {
    /// the requested saved notecard's metadata (display name, owner,
    /// scope) — the raw body is fetched separately via the `/text`
    /// sub-route.
    pub notecard: NotecardView,
}

/// `GET /api/notecards?scope=...` — list notecards in a scope.
///
/// # Errors
///
/// Returns [`Error::BadRequest`] for an invalid scope; [`Error::Forbidden`]
/// if the user is not allowed to view that scope.
pub async fn list(
    user: CurrentUser,
    State(state): State<AppState>,
    Query(query): Query<ScopeQuery>,
) -> Result<Json<ListNotecardsResponse>, Error> {
    let destination = Destination::parse(&query.scope)?;
    library::assert_can_view(&state.db, user.user_id, destination).await?;
    let notecards = fetch_notecards_for(&state, user.user_id, destination).await?;
    Ok(Json(ListNotecardsResponse { notecards }))
}

/// `POST /api/notecards` — save a fresh notecard. Multipart body:
/// `notecard` (file) or `notecard_text` (textarea), `name`, `destination`.
///
/// # Errors
///
/// Returns [`Error::BadRequest`] for malformed input; [`Error::Forbidden`]
/// for writing to a group the user does not own.
pub async fn create(
    user: CurrentUser,
    State(state): State<AppState>,
    multipart: Multipart,
) -> Result<(ReqwestStatusCode, Json<NotecardResponse>), Error> {
    rate_limit::try_acquire(&state.db, RateCategory::NotecardCreate, user.user_id).await?;
    let parsed = parse_create_form(multipart).await?;
    library::assert_can_write(&state.db, user.user_id, parsed.destination).await?;

    let notecard_id = insert_notecard_row(
        &state,
        parsed.destination,
        user.user_id,
        &parsed.name,
        &parsed.text,
    )
    .await?;
    let (start_region, end_region, waypoint_count) = notecard_summary(&parsed.text);
    let view = build_view(
        notecard_id,
        parsed.destination,
        user.user_id,
        &user.username,
        &user.legacy_name,
        &parsed.name,
        Utc::now(),
        NotecardExtras {
            start_region,
            end_region,
            waypoint_count,
            ..NotecardExtras::default()
        },
    );
    Ok((
        ReqwestStatusCode::CREATED,
        Json(NotecardResponse { notecard: view }),
    ))
}

/// `GET /api/notecards/{id}` — fetch a notecard's metadata.
///
/// # Errors
///
/// Returns [`Error::Forbidden`] if the caller may not view it;
/// [`Error::NotFound`] if it doesn't exist.
pub async fn get(
    user: CurrentUser,
    State(state): State<AppState>,
    Path(notecard_id): Path<Uuid>,
) -> Result<Json<NotecardResponse>, Error> {
    let row = library::assert_can_read_notecard(&state.db, user.user_id, notecard_id).await?;
    let destination =
        library::destination_from_columns(row.owner_user_id.clone(), row.owner_group_id.clone())?;
    let (uploader_username, uploader_legacy) = lookup_user_names(&state, row.uploaded_by).await?;
    let (start_region, end_region, waypoint_count) = notecard_summary(&row.body);
    let view = build_view(
        notecard_id,
        destination,
        row.uploaded_by,
        &uploader_username,
        &uploader_legacy,
        &row.name,
        row.created_at,
        NotecardExtras {
            start_region,
            end_region,
            waypoint_count,
            lower_left_x: row.lower_left_x,
            lower_left_y: row.lower_left_y,
            upper_right_x: row.upper_right_x,
            upper_right_y: row.upper_right_y,
        },
    );
    Ok(Json(NotecardResponse { notecard: view }))
}

/// `GET /api/notecards/{id}/text` — download the raw notecard text.
///
/// # Errors
///
/// As [`get`].
pub async fn download_text(
    user: CurrentUser,
    State(state): State<AppState>,
    Path(notecard_id): Path<Uuid>,
) -> Result<Response, Error> {
    let row = library::assert_can_read_notecard(&state.db, user.user_id, notecard_id).await?;
    let filename = format!(
        "{}.txt",
        sanitise_for_filename(&row.name).unwrap_or_else(|| notecard_id.to_string())
    );
    let disposition = format!("attachment; filename=\"{filename}\"");
    let body = row.body;
    let headers = [
        (header::CONTENT_TYPE, "text/plain; charset=utf-8".to_owned()),
        (header::CONTENT_DISPOSITION, disposition),
    ];
    Ok((headers, body).into_response())
}

/// `DELETE /api/notecards/{id}` — delete a notecard. Personal owner or
/// group owner only. Returns a friendly 400 if a render still references it
/// (FK RESTRICT).
///
/// # Errors
///
/// Returns [`Error::Forbidden`] for permission failure, [`Error::BadRequest`]
/// for FK violations, or [`Error::Database`] for other DB failures.
pub async fn delete(
    user: CurrentUser,
    State(state): State<AppState>,
    Path(notecard_id): Path<Uuid>,
) -> Result<Response, Error> {
    library::assert_can_delete_notecard(&state.db, user.user_id, notecard_id).await?;
    let result = sqlx::query("DELETE FROM saved_notecards WHERE notecard_id = ?1")
        .bind(notecard_id.as_bytes().to_vec())
        .execute(&state.db)
        .await;
    match result {
        Ok(_) => Ok((ReqwestStatusCode::NO_CONTENT, "").into_response()),
        Err(err) => {
            if error::is_fk_violation(&err) {
                return Err(Error::BadRequest(
                    "cannot delete this notecard: one or more saved renders still reference it. \
                     Delete those renders first."
                        .to_owned(),
                ));
            }
            tracing::error!("notecard delete failed: {err}");
            Err(Error::Database)
        }
    }
}

/// Parsed `POST /api/notecards` form fields.
struct CreateForm {
    /// destination scope (personal or a group).
    destination: Destination,
    /// the human-supplied display name.
    name: String,
    /// the raw notecard text (validated by parsing through `USBNotecard`).
    text: String,
}

/// Parse the multipart form for a `POST /api/notecards` request.
async fn parse_create_form(mut multipart: Multipart) -> Result<CreateForm, Error> {
    let mut text_from_file: Option<String> = None;
    let mut text_pasted: Option<String> = None;
    let mut name: Option<String> = None;
    let mut destination_raw: Option<String> = None;
    while let Some(field) = multipart.next_field().await? {
        let Some(field_name) = field.name().map(str::to_owned) else {
            continue;
        };
        match field_name.as_str() {
            "notecard" => {
                let bytes = field.bytes().await?;
                if !bytes.is_empty() {
                    text_from_file =
                        Some(String::from_utf8(bytes.to_vec()).map_err(|e| {
                            Error::BadRequest(format!("notecard is not UTF-8: {e}"))
                        })?);
                }
            }
            "notecard_text" => {
                let text = field.text().await?;
                if !text.trim().is_empty() {
                    text_pasted = Some(text);
                }
            }
            "name" => {
                let t = field.text().await?;
                if !t.trim().is_empty() {
                    name = Some(t);
                }
            }
            "destination" => destination_raw = Some(field.text().await?),
            _ => {}
        }
    }
    let raw = text_from_file
        .or(text_pasted)
        .ok_or_else(|| Error::BadRequest("supply a notecard file or text".to_owned()))?;
    // Validate by parsing; we still store the raw text for fidelity (the
    // upstream USBNotecard type may strip whitespace etc. during parse).
    let _parsed: USBNotecard = raw.parse()?;
    let name_raw = name.ok_or_else(|| Error::BadRequest("name is required".to_owned()))?;
    let name = library::sanitise_display_name(&name_raw, "notecard name")?;
    let destination = Destination::parse(destination_raw.as_deref().unwrap_or("personal"))?;
    Ok(CreateForm {
        destination,
        name,
        text: raw,
    })
}

/// Insert a `saved_notecards` row and return its new id.
pub(crate) async fn insert_notecard_row(
    state: &AppState,
    destination: Destination,
    uploaded_by: Uuid,
    name: &str,
    text: &str,
) -> Result<Uuid, Error> {
    let notecard_id = Uuid::new_v4();
    let now = Utc::now();
    let (owner_user, owner_group) = match destination {
        Destination::Personal => (Some(uploaded_by.as_bytes().to_vec()), None),
        Destination::Group { group_id } => (None, Some(group_id.as_bytes().to_vec())),
    };
    sqlx::query(
        "INSERT INTO saved_notecards \
            (notecard_id, owner_user_id, owner_group_id, uploaded_by, name, body, created_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
    )
    .bind(notecard_id.as_bytes().to_vec())
    .bind(owner_user)
    .bind(owner_group)
    .bind(uploaded_by.as_bytes().to_vec())
    .bind(name)
    .bind(text)
    .bind(now)
    .execute(&state.db)
    .await
    .map_err(|err| {
        tracing::error!("insert saved_notecards failed: {err}");
        Error::Database
    })?;
    Ok(notecard_id)
}

/// Bundle of optional notecard-row fields that are not part of the core
/// view but are surfaced through the API for the library UI.
#[derive(Default)]
struct NotecardExtras {
    /// route start region (first waypoint's region name).
    start_region: Option<String>,
    /// route end region (last waypoint's region name).
    end_region: Option<String>,
    /// number of waypoints in the route.
    waypoint_count: Option<u32>,
    /// lower-left x grid coordinate of the route's bounding box.
    lower_left_x: Option<u16>,
    /// lower-left y grid coordinate of the route's bounding box.
    lower_left_y: Option<u16>,
    /// upper-right x grid coordinate of the route's bounding box.
    upper_right_x: Option<u16>,
    /// upper-right y grid coordinate of the route's bounding box.
    upper_right_y: Option<u16>,
}

/// Derive the start/end region names and waypoint count from a notecard
/// body. All three are `None` if the body fails to parse; start/end are
/// also `None` if the parsed route has no waypoints. The body has
/// already been validated at insert time so parse failures here would
/// indicate corruption, but we still degrade gracefully rather than
/// 500ing the list endpoint.
fn notecard_summary(body: &str) -> (Option<String>, Option<String>, Option<u32>) {
    let Ok(parsed) = body.parse::<USBNotecard>() else {
        return (None, None, None);
    };
    let waypoints = parsed.waypoints();
    let start = waypoints
        .first()
        .map(|w| w.location().region_name.to_string());
    let end = waypoints
        .last()
        .map(|w| w.location().region_name.to_string());
    let count = u32::try_from(waypoints.len()).ok();
    (start, end, count)
}

/// Convert a raw SQLite `INTEGER` from the bounds columns to `u16`.
/// Returns `None` for `NULL` or for values that fall outside the grid's
/// `u16` range (which the schema does not constrain but the renderer
/// would reject anyway).
fn bound_u16(v: Option<i64>) -> Option<u16> {
    v.and_then(|n| u16::try_from(n).ok())
}

/// Build a [`NotecardView`] from the gathered pieces.
#[expect(
    clippy::too_many_arguments,
    reason = "every argument is a distinct view field; bundling them would add indirection without removing parameters"
)]
fn build_view(
    notecard_id: Uuid,
    destination: Destination,
    uploaded_by: Uuid,
    uploaded_by_username: &str,
    uploaded_by_legacy_name: &str,
    name: &str,
    created_at: DateTime<Utc>,
    extras: NotecardExtras,
) -> NotecardView {
    NotecardView {
        notecard_id,
        destination,
        uploaded_by,
        uploaded_by_username: uploaded_by_username.to_owned(),
        uploaded_by_legacy_name: uploaded_by_legacy_name.to_owned(),
        name: name.to_owned(),
        created_at,
        start_region: extras.start_region,
        end_region: extras.end_region,
        waypoint_count: extras.waypoint_count,
        lower_left_x: extras.lower_left_x,
        lower_left_y: extras.lower_left_y,
        upper_right_x: extras.upper_right_x,
        upper_right_y: extras.upper_right_y,
    }
}

/// Row shape returned by the listing queries for `saved_notecards`. A
/// `FromRow` struct is used instead of a tuple so we can grow the column
/// list without worrying about sqlx's tuple-`FromRow` arity (16) and so
/// the field-by-field destructuring stays readable.
#[derive(sqlx::FromRow)]
struct NotecardListRow {
    /// raw bytes of `saved_notecards.notecard_id`.
    notecard_id: Vec<u8>,
    /// raw bytes of `saved_notecards.owner_user_id`, if set.
    owner_user_id: Option<Vec<u8>>,
    /// raw bytes of `saved_notecards.owner_group_id`, if set.
    owner_group_id: Option<Vec<u8>>,
    /// raw bytes of the uploading user's id.
    uploaded_by: Vec<u8>,
    /// the uploading user's `username`.
    uploader_username: String,
    /// the uploading user's `legacy_name`.
    uploader_legacy: String,
    /// notecard display name.
    name: String,
    /// the raw notecard body (used to extract the start / end region
    /// names; not returned to the client through the list response).
    body: String,
    /// row creation timestamp.
    created_at: DateTime<Utc>,
    /// lower-left x grid coordinate of the route's bounding box, if a
    /// previous render has resolved and cached it.
    lower_left_x: Option<i64>,
    /// lower-left y grid coordinate of the route's bounding box.
    lower_left_y: Option<i64>,
    /// upper-right x grid coordinate of the route's bounding box.
    upper_right_x: Option<i64>,
    /// upper-right y grid coordinate of the route's bounding box.
    upper_right_y: Option<i64>,
}

/// Run the list query for the given scope.
async fn fetch_notecards_for(
    state: &AppState,
    current_user: Uuid,
    destination: Destination,
) -> Result<Vec<NotecardView>, Error> {
    let rows: Vec<NotecardListRow> = match destination {
        Destination::Personal => sqlx::query_as(
            "SELECT n.notecard_id, n.owner_user_id, n.owner_group_id, n.uploaded_by AS uploaded_by, \
                    u.username AS uploader_username, u.legacy_name AS uploader_legacy, \
                    n.name, n.body, n.created_at, \
                    n.lower_left_x, n.lower_left_y, n.upper_right_x, n.upper_right_y \
             FROM saved_notecards AS n \
             JOIN users AS u ON u.user_id = n.uploaded_by \
             WHERE n.owner_user_id = ?1 \
             ORDER BY n.created_at DESC",
        )
        .bind(current_user.as_bytes().to_vec())
        .fetch_all(&state.db)
        .await
        .map_err(|err| {
            tracing::error!("list notecards (personal) failed: {err}");
            Error::Database
        })?,
        Destination::Group { group_id } => sqlx::query_as(
            // The JOIN against `group_memberships` enforces visibility at the
            // SQL layer so a forgotten `assert_can_view` call site cannot leak
            // a group's notecards to a non-member.
            "SELECT n.notecard_id, n.owner_user_id, n.owner_group_id, n.uploaded_by AS uploaded_by, \
                    u.username AS uploader_username, u.legacy_name AS uploader_legacy, \
                    n.name, n.body, n.created_at, \
                    n.lower_left_x, n.lower_left_y, n.upper_right_x, n.upper_right_y \
             FROM saved_notecards AS n \
             JOIN users AS u ON u.user_id = n.uploaded_by \
             JOIN group_memberships AS gm \
               ON gm.group_id = n.owner_group_id AND gm.user_id = ?2 \
             WHERE n.owner_group_id = ?1 \
             ORDER BY n.created_at DESC",
        )
        .bind(group_id.as_bytes().to_vec())
        .bind(current_user.as_bytes().to_vec())
        .fetch_all(&state.db)
        .await
        .map_err(|err| {
            tracing::error!("list notecards (group) failed: {err}");
            Error::Database
        })?,
    };
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        let notecard_id = uuid_from_bytes(&row.notecard_id).ok_or_else(|| {
            tracing::error!("bad notecard uuid");
            Error::Database
        })?;
        let row_dest = library::destination_from_columns(row.owner_user_id, row.owner_group_id)?;
        let uploaded_by = uuid_from_bytes(&row.uploaded_by).ok_or_else(|| {
            tracing::error!("bad uploaded_by uuid");
            Error::Database
        })?;
        let (start_region, end_region, waypoint_count) = notecard_summary(&row.body);
        out.push(NotecardView {
            notecard_id,
            destination: row_dest,
            uploaded_by,
            uploaded_by_username: row.uploader_username,
            uploaded_by_legacy_name: row.uploader_legacy,
            name: row.name,
            created_at: row.created_at,
            start_region,
            end_region,
            waypoint_count,
            lower_left_x: bound_u16(row.lower_left_x),
            lower_left_y: bound_u16(row.lower_left_y),
            upper_right_x: bound_u16(row.upper_right_x),
            upper_right_y: bound_u16(row.upper_right_y),
        });
    }
    Ok(out)
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

/// Reduce a free-form notecard name to something safe to embed inside a
/// Content-Disposition filename. Returns None if nothing usable is left.
pub(crate) fn sanitise_for_filename(name: &str) -> Option<String> {
    let cleaned: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let trimmed = cleaned.trim_matches('_').to_owned();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}
