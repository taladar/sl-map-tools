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
    /// the notecards.
    pub notecards: Vec<NotecardView>,
}

/// Response carrying a single saved notecard's metadata.
#[derive(Debug, Serialize)]
pub struct NotecardResponse {
    /// the notecard.
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
    let view = build_view(
        notecard_id,
        parsed.destination,
        user.user_id,
        &user.username,
        &user.legacy_name,
        &parsed.name,
        Utc::now(),
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
    let view = build_view(
        notecard_id,
        destination,
        row.uploaded_by,
        &uploader_username,
        &uploader_legacy,
        &row.name,
        row.created_at,
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

/// Build a [`NotecardView`] from the gathered pieces.
fn build_view(
    notecard_id: Uuid,
    destination: Destination,
    uploaded_by: Uuid,
    uploaded_by_username: &str,
    uploaded_by_legacy_name: &str,
    name: &str,
    created_at: DateTime<Utc>,
) -> NotecardView {
    NotecardView {
        notecard_id,
        destination,
        uploaded_by,
        uploaded_by_username: uploaded_by_username.to_owned(),
        uploaded_by_legacy_name: uploaded_by_legacy_name.to_owned(),
        name: name.to_owned(),
        created_at,
    }
}

/// Tuple shape returned by the listing queries for `saved_notecards`.
type NotecardListRow = (
    Vec<u8>,
    Option<Vec<u8>>,
    Option<Vec<u8>>,
    Vec<u8>,
    String,
    String,
    String,
    DateTime<Utc>,
);

/// Run the list query for the given scope.
async fn fetch_notecards_for(
    state: &AppState,
    current_user: Uuid,
    destination: Destination,
) -> Result<Vec<NotecardView>, Error> {
    let rows: Vec<NotecardListRow> = match destination {
        Destination::Personal => sqlx::query_as(
            "SELECT n.notecard_id, n.owner_user_id, n.owner_group_id, n.uploaded_by, \
                    u.username, u.legacy_name, n.name, n.created_at \
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
            "SELECT n.notecard_id, n.owner_user_id, n.owner_group_id, n.uploaded_by, \
                    u.username, u.legacy_name, n.name, n.created_at \
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
    for (
        nid_bytes,
        owner_user,
        owner_group,
        uploaded_by_bytes,
        uploader_username,
        uploader_legacy,
        name,
        created_at,
    ) in rows
    {
        let notecard_id = uuid_from_bytes(&nid_bytes).ok_or_else(|| {
            tracing::error!("bad notecard uuid");
            Error::Database
        })?;
        let row_dest = library::destination_from_columns(owner_user, owner_group)?;
        let uploaded_by = uuid_from_bytes(&uploaded_by_bytes).ok_or_else(|| {
            tracing::error!("bad uploaded_by uuid");
            Error::Database
        })?;
        out.push(NotecardView {
            notecard_id,
            destination: row_dest,
            uploaded_by,
            uploaded_by_username: uploader_username,
            uploaded_by_legacy_name: uploader_legacy,
            name,
            created_at,
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
fn sanitise_for_filename(name: &str) -> Option<String> {
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
