//! HTTP handlers for saved logo images.
//!
//! Logos are uploaded by users into a personal or group library, stored as
//! files under `<storage_dir>/logos/`, and later composited onto a render at
//! a free placement slot (see [`crate::routes::render`]). The CRUD routes
//! below let users upload / list / inspect / download / rename / delete them,
//! following the same shape as [`crate::routes::glw`] and
//! [`crate::routes::notecards`].

use axum::Json;
use axum::extract::{Multipart, Path, Query, State};
use axum::http::StatusCode as ReqwestStatusCode;
use axum::http::header;
use axum::response::{IntoResponse as _, Response};
use chrono::{DateTime, Utc};
use image::GenericImageView as _;
use serde::{Deserialize, Serialize};
use std::sync::atomic::Ordering;
use uuid::Uuid;

use crate::auth::{CurrentUser, uuid_from_bytes};
use crate::error::{self, Error};
use crate::library::{self, Destination, LogoView};
use crate::state::AppState;
use crate::storage;

/// Maximum accepted size of an uploaded logo file, in bytes (5 MiB).
const MAX_LOGO_BYTES: usize = 5 * 1024 * 1024;

/// Maximum accepted width or height of an uploaded logo, in pixels. Matches
/// the larger of the two Second Life texture sizes.
const MAX_LOGO_DIMENSION: u32 = 2048;

/// Query parameters for `GET /api/logos`.
#[derive(Debug, Deserialize)]
pub struct ScopeQuery {
    /// `"personal"` or `"group:<uuid>"`.
    pub scope: String,
}

/// Response carrying the listing of logos in a scope.
#[derive(Debug, Serialize)]
pub struct ListLogosResponse {
    /// the saved logos visible in the requested scope, newest first.
    pub logos: Vec<LogoView>,
}

/// Response carrying a single saved logo's metadata.
#[derive(Debug, Serialize)]
pub struct LogoResponse {
    /// the requested logo's metadata. The bytes are downloaded separately
    /// via `GET /api/logos/{id}/image`.
    pub logo: LogoView,
}

/// `GET /api/logos?scope=…` — list logos in a scope.
///
/// # Errors
///
/// Returns [`Error::BadRequest`] for an invalid scope; [`Error::Forbidden`]
/// if the user is not allowed to view that scope.
pub async fn list(
    user: CurrentUser,
    State(state): State<AppState>,
    Query(query): Query<ScopeQuery>,
) -> Result<Json<ListLogosResponse>, Error> {
    let destination = Destination::parse(&query.scope)?;
    library::assert_can_view(&state.db, user.user_id, destination).await?;
    let logos = fetch_logos_for(&state, user.user_id, destination).await?;
    Ok(Json(ListLogosResponse { logos }))
}

/// `POST /api/logos` — upload a fresh logo. Multipart body: `file` (the
/// image), `name`, `scope`.
///
/// # Errors
///
/// Returns [`Error::BadRequest`] for a missing/oversized/undecodable image or
/// malformed fields; [`Error::Forbidden`] for writing to a group the user
/// does not own.
pub async fn create(
    user: CurrentUser,
    State(state): State<AppState>,
    multipart: Multipart,
) -> Result<(ReqwestStatusCode, Json<LogoResponse>), Error> {
    let parsed = parse_create_form(multipart).await?;
    library::assert_can_write(&state.db, user.user_id, parsed.destination).await?;

    let logo_id = Uuid::new_v4();
    let ext = storage::ext_for_content_type(parsed.content_type);
    let byte_size = parsed.bytes.len();
    let image_filename =
        match storage::write_logo_file(&state.config.storage_dir, logo_id, ext, parsed.bytes).await
        {
            Ok(f) => f,
            Err(err) => {
                tracing::error!("write logo file failed: {err}");
                return Err(Error::Io(std::io::Error::other("could not store logo")));
            }
        };

    let now = Utc::now();
    if let Err(err) = insert_logo_row(
        &state,
        &InsertLogo {
            logo_id,
            destination: parsed.destination,
            uploaded_by: user.user_id,
            name: &parsed.name,
            content_type: parsed.content_type,
            image_filename: &image_filename,
            width: parsed.width,
            height: parsed.height,
            byte_size,
            created_at: now,
        },
    )
    .await
    {
        // The row never landed; flag the orphaned file for the sweeper.
        state.library_cleanup_dirty.store(true, Ordering::Release);
        return Err(err);
    }

    let view = build_view(
        logo_id,
        parsed.destination,
        Some(user.user_id),
        Some(&user.username),
        Some(&user.legacy_name),
        &parsed.name,
        parsed.content_type,
        parsed.width,
        parsed.height,
        byte_size,
        now,
    );
    Ok((
        ReqwestStatusCode::CREATED,
        Json(LogoResponse { logo: view }),
    ))
}

/// `GET /api/logos/{id}` — fetch a logo's metadata.
///
/// # Errors
///
/// Returns [`Error::NotFound`] if it does not exist or is invisible.
pub async fn get(
    user: CurrentUser,
    State(state): State<AppState>,
    Path(logo_id): Path<Uuid>,
) -> Result<Json<LogoResponse>, Error> {
    let row = library::assert_can_read_logo(&state.db, user.user_id, logo_id).await?;
    let destination =
        library::destination_from_columns(row.owner_user_id.clone(), row.owner_group_id.clone())?;
    let (uploader_username, uploader_legacy) = match row.uploaded_by {
        Some(id) => match lookup_user_names(&state, id).await {
            Ok((u, l)) => (Some(u), Some(l)),
            Err(_) => (None, None),
        },
        None => (None, None),
    };
    let view = build_view(
        logo_id,
        destination,
        row.uploaded_by,
        uploader_username.as_deref(),
        uploader_legacy.as_deref(),
        &row.name,
        &row.content_type,
        row.width,
        row.height,
        usize::try_from(row.byte_size).unwrap_or(usize::MAX),
        row.created_at,
    );
    Ok(Json(LogoResponse { logo: view }))
}

/// `GET /api/logos/{id}/image` — serve the logo's raw image bytes inline so
/// the library thumbnail and the map-editor picker can display them.
///
/// # Errors
///
/// Returns [`Error::NotFound`] if the logo does not exist or is invisible.
pub async fn image(
    user: CurrentUser,
    State(state): State<AppState>,
    Path(logo_id): Path<Uuid>,
) -> Result<Response, Error> {
    let row = library::assert_can_read_logo(&state.db, user.user_id, logo_id).await?;
    let bytes = storage::read_logo_file(&state.config.storage_dir, &row.image_filename).await?;
    let headers = [(header::CONTENT_TYPE, row.content_type)];
    Ok((headers, bytes).into_response())
}

/// Body for `PATCH /api/logos/{id}` — only the display `name` can change;
/// the bytes are immutable so any render referencing the logo keeps pointing
/// at the same image.
#[derive(Debug, Deserialize)]
pub struct RenameRequest {
    /// new display name.
    pub name: String,
}

/// `PATCH /api/logos/{id}` — rename a saved logo. Personal owner or group
/// owner only.
///
/// # Errors
///
/// Returns [`Error::Forbidden`] / [`Error::NotFound`] / [`Error::BadRequest`].
pub async fn rename(
    user: CurrentUser,
    State(state): State<AppState>,
    Path(logo_id): Path<Uuid>,
    Json(body): Json<RenameRequest>,
) -> Result<Json<LogoResponse>, Error> {
    let trimmed = library::sanitise_display_name(&body.name, "name")?;
    let row = library::assert_can_delete_logo(&state.db, user.user_id, logo_id).await?;
    sqlx::query("UPDATE saved_logos SET name = ?1 WHERE logo_id = ?2")
        .bind(&trimmed)
        .bind(logo_id.as_bytes().to_vec())
        .execute(&state.db)
        .await
        .map_err(|err| {
            tracing::error!("logo rename failed: {err}");
            Error::Database
        })?;
    let destination =
        library::destination_from_columns(row.owner_user_id.clone(), row.owner_group_id.clone())?;
    let (uploader_username, uploader_legacy) = match row.uploaded_by {
        Some(id) => match lookup_user_names(&state, id).await {
            Ok((u, l)) => (Some(u), Some(l)),
            Err(_) => (None, None),
        },
        None => (None, None),
    };
    let view = build_view(
        logo_id,
        destination,
        row.uploaded_by,
        uploader_username.as_deref(),
        uploader_legacy.as_deref(),
        &trimmed,
        &row.content_type,
        row.width,
        row.height,
        usize::try_from(row.byte_size).unwrap_or(usize::MAX),
        row.created_at,
    );
    Ok(Json(LogoResponse { logo: view }))
}

/// `DELETE /api/logos/{id}` — delete a saved logo. Personal owner or group
/// owner only. The FK `saved_render_logos.logo_id` is `ON DELETE RESTRICT`,
/// so a logo used by any render fails with a human-friendly error.
///
/// # Errors
///
/// Returns [`Error::Forbidden`] / [`Error::NotFound`] / [`Error::BadRequest`].
pub async fn delete(
    user: CurrentUser,
    State(state): State<AppState>,
    Path(logo_id): Path<Uuid>,
) -> Result<Response, Error> {
    let row = library::assert_can_delete_logo(&state.db, user.user_id, logo_id).await?;
    let result = sqlx::query("DELETE FROM saved_logos WHERE logo_id = ?1")
        .bind(logo_id.as_bytes().to_vec())
        .execute(&state.db)
        .await;
    match result {
        Ok(_) => {
            // Best-effort unlink; a failure just leaves the file for the
            // orphan sweeper to collect.
            if let Err(err) =
                storage::try_delete_logo_file(&state.config.storage_dir, &row.image_filename)
            {
                tracing::warn!("could not unlink logo file {}: {err}", row.image_filename);
                state.library_cleanup_dirty.store(true, Ordering::Release);
            }
            Ok((ReqwestStatusCode::NO_CONTENT, "").into_response())
        }
        Err(err) => {
            if error::is_fk_violation(&err) {
                return Err(Error::BadRequest(
                    "cannot delete this logo: one or more saved renders still reference it. \
                     Delete those renders first."
                        .to_owned(),
                ));
            }
            tracing::error!("logo delete failed: {err}");
            Err(Error::Database)
        }
    }
}

/// Parsed `POST /api/logos` form fields plus the validated image.
struct CreateForm {
    /// destination scope (personal or a group).
    destination: Destination,
    /// the human-supplied display name.
    name: String,
    /// canonical MIME type derived from the decoded image bytes.
    content_type: &'static str,
    /// intrinsic image width in pixels.
    width: u32,
    /// intrinsic image height in pixels.
    height: u32,
    /// the raw image bytes to store.
    bytes: bytes::Bytes,
}

/// Map a decoded [`image::ImageFormat`] to the canonical stored MIME type,
/// rejecting anything that is not one of the three accepted formats.
fn content_type_for_format(format: image::ImageFormat) -> Result<&'static str, Error> {
    match format {
        image::ImageFormat::Png => Ok("image/png"),
        image::ImageFormat::Jpeg => Ok("image/jpeg"),
        image::ImageFormat::WebP => Ok("image/webp"),
        other => Err(Error::BadRequest(format!(
            "unsupported image format {other:?}; upload a PNG, JPEG or WebP"
        ))),
    }
}

/// Parse the multipart form for a `POST /api/logos` request, validating the
/// uploaded image's format, size and dimensions.
async fn parse_create_form(mut multipart: Multipart) -> Result<CreateForm, Error> {
    let mut file_bytes: Option<bytes::Bytes> = None;
    let mut name: Option<String> = None;
    let mut scope_raw: Option<String> = None;
    while let Some(field) = multipart.next_field().await? {
        let Some(field_name) = field.name().map(str::to_owned) else {
            continue;
        };
        match field_name.as_str() {
            "file" => {
                let bytes = field.bytes().await?;
                if !bytes.is_empty() {
                    file_bytes = Some(bytes);
                }
            }
            "name" => {
                let t = field.text().await?;
                if !t.trim().is_empty() {
                    name = Some(t);
                }
            }
            "scope" => scope_raw = Some(field.text().await?),
            _ => {}
        }
    }

    let bytes =
        file_bytes.ok_or_else(|| Error::BadRequest("supply a logo image file".to_owned()))?;
    if bytes.len() > MAX_LOGO_BYTES {
        return Err(Error::BadRequest(format!(
            "logo image is {} bytes; the maximum is {MAX_LOGO_BYTES} bytes",
            bytes.len()
        )));
    }
    // Determine the format from the bytes themselves rather than trusting the
    // multipart content-type header, then decode to confirm it is a valid
    // image and to read its dimensions.
    let format = image::guess_format(&bytes)
        .map_err(|e| Error::BadRequest(format!("could not recognise image format: {e}")))?;
    let content_type = content_type_for_format(format)?;
    let decoded = image::load_from_memory_with_format(&bytes, format)
        .map_err(|e| Error::BadRequest(format!("could not decode image: {e}")))?;
    let (width, height) = decoded.dimensions();
    if width == 0 || height == 0 {
        return Err(Error::BadRequest("logo image has zero size".to_owned()));
    }
    if width > MAX_LOGO_DIMENSION || height > MAX_LOGO_DIMENSION {
        return Err(Error::BadRequest(format!(
            "logo image is {width}x{height}px; each side must be at most {MAX_LOGO_DIMENSION}px"
        )));
    }

    let name_raw = name.ok_or_else(|| Error::BadRequest("name is required".to_owned()))?;
    let name = library::sanitise_display_name(&name_raw, "logo name")?;
    let destination = Destination::parse(scope_raw.as_deref().unwrap_or("personal"))?;
    Ok(CreateForm {
        destination,
        name,
        content_type,
        width,
        height,
        bytes,
    })
}

/// Fields needed to persist a fresh `saved_logos` row.
struct InsertLogo<'a> {
    /// the pre-generated logo id (also the on-disk filename stem).
    logo_id: Uuid,
    /// destination scope (personal or a group).
    destination: Destination,
    /// the avatar uploading the logo.
    uploaded_by: Uuid,
    /// the display name.
    name: &'a str,
    /// canonical MIME type.
    content_type: &'a str,
    /// relative filename under `<storage_dir>/logos/`.
    image_filename: &'a str,
    /// intrinsic image width in pixels.
    width: u32,
    /// intrinsic image height in pixels.
    height: u32,
    /// size of the stored bytes.
    byte_size: usize,
    /// row creation timestamp.
    created_at: DateTime<Utc>,
}

/// Persist a fresh `saved_logos` row. Callers must already have permission to
/// write to the destination.
///
/// # Errors
///
/// Returns [`Error::Database`] on any underlying SQLite failure.
async fn insert_logo_row(state: &AppState, insert: &InsertLogo<'_>) -> Result<(), Error> {
    let (owner_user, owner_group) = match insert.destination {
        Destination::Personal => (Some(insert.uploaded_by.as_bytes().to_vec()), None),
        Destination::Group { group_id } => (None, Some(group_id.as_bytes().to_vec())),
    };
    sqlx::query(
        "INSERT INTO saved_logos \
            (logo_id, owner_user_id, owner_group_id, uploaded_by, name, \
             content_type, image_filename, width, height, byte_size, created_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
    )
    .bind(insert.logo_id.as_bytes().to_vec())
    .bind(owner_user)
    .bind(owner_group)
    .bind(insert.uploaded_by.as_bytes().to_vec())
    .bind(insert.name)
    .bind(insert.content_type)
    .bind(insert.image_filename)
    .bind(i64::from(insert.width))
    .bind(i64::from(insert.height))
    .bind(i64::try_from(insert.byte_size).unwrap_or(i64::MAX))
    .bind(insert.created_at)
    .execute(&state.db)
    .await
    .map_err(|err| {
        tracing::error!("insert saved_logos failed: {err}");
        Error::Database
    })?;
    Ok(())
}

/// Row shape returned by the listing query.
#[derive(sqlx::FromRow)]
struct LogoListRow {
    /// raw bytes of `saved_logos.logo_id`.
    logo_id: Vec<u8>,
    /// raw bytes of `saved_logos.owner_user_id`, if set.
    owner_user_id: Option<Vec<u8>>,
    /// raw bytes of `saved_logos.owner_group_id`, if set.
    owner_group_id: Option<Vec<u8>>,
    /// raw bytes of the uploading user's id, if the account still exists.
    uploaded_by: Option<Vec<u8>>,
    /// the uploading user's `username`.
    uploader_username: Option<String>,
    /// the uploading user's `legacy_name`.
    uploader_legacy: Option<String>,
    /// display name.
    name: String,
    /// MIME type of the stored bytes.
    content_type: String,
    /// intrinsic image width in pixels.
    width: i64,
    /// intrinsic image height in pixels.
    height: i64,
    /// size of the stored bytes.
    byte_size: i64,
    /// row creation timestamp.
    created_at: DateTime<Utc>,
}

/// Run the list query for the given scope.
async fn fetch_logos_for(
    state: &AppState,
    current_user: Uuid,
    destination: Destination,
) -> Result<Vec<LogoView>, Error> {
    let rows: Vec<LogoListRow> = match destination {
        Destination::Personal => sqlx::query_as(
            "SELECT l.logo_id, l.owner_user_id, l.owner_group_id, l.uploaded_by AS uploaded_by, \
                    u.username AS uploader_username, u.legacy_name AS uploader_legacy, \
                    l.name, l.content_type, l.width, l.height, l.byte_size, l.created_at \
             FROM saved_logos AS l \
             LEFT JOIN users AS u ON u.user_id = l.uploaded_by \
             WHERE l.owner_user_id = ?1 \
             ORDER BY l.created_at DESC",
        )
        .bind(current_user.as_bytes().to_vec())
        .fetch_all(&state.db)
        .await
        .map_err(|err| {
            tracing::error!("list logos (personal) failed: {err}");
            Error::Database
        })?,
        Destination::Group { group_id } => sqlx::query_as(
            // The JOIN against group_memberships enforces visibility at the
            // SQL layer, matching the other library list queries.
            "SELECT l.logo_id, l.owner_user_id, l.owner_group_id, l.uploaded_by AS uploaded_by, \
                    u.username AS uploader_username, u.legacy_name AS uploader_legacy, \
                    l.name, l.content_type, l.width, l.height, l.byte_size, l.created_at \
             FROM saved_logos AS l \
             LEFT JOIN users AS u ON u.user_id = l.uploaded_by \
             JOIN group_memberships AS gm \
               ON gm.group_id = l.owner_group_id AND gm.user_id = ?2 \
             WHERE l.owner_group_id = ?1 \
             ORDER BY l.created_at DESC",
        )
        .bind(group_id.as_bytes().to_vec())
        .bind(current_user.as_bytes().to_vec())
        .fetch_all(&state.db)
        .await
        .map_err(|err| {
            tracing::error!("list logos (group) failed: {err}");
            Error::Database
        })?,
    };
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        let logo_id = uuid_from_bytes(&row.logo_id).ok_or_else(|| {
            tracing::error!("bad logo uuid");
            Error::Database
        })?;
        let row_dest = library::destination_from_columns(row.owner_user_id, row.owner_group_id)?;
        let uploaded_by = row
            .uploaded_by
            .as_deref()
            .map(uuid_from_bytes)
            .map(|opt| {
                opt.ok_or_else(|| {
                    tracing::error!("bad uploaded_by uuid in saved_logos");
                    Error::Database
                })
            })
            .transpose()?;
        out.push(LogoView {
            logo_id,
            destination: row_dest,
            uploaded_by,
            uploaded_by_username: row.uploader_username,
            uploaded_by_legacy_name: row.uploader_legacy,
            name: row.name,
            content_type: row.content_type,
            width: u32::try_from(row.width).unwrap_or(0),
            height: u32::try_from(row.height).unwrap_or(0),
            byte_size: u64::try_from(row.byte_size).unwrap_or(0),
            created_at: row.created_at,
        });
    }
    Ok(out)
}

/// Build a [`LogoView`] from the gathered pieces.
#[expect(
    clippy::too_many_arguments,
    reason = "every argument is a distinct view field; bundling them would add indirection without removing parameters"
)]
fn build_view(
    logo_id: Uuid,
    destination: Destination,
    uploaded_by: Option<Uuid>,
    uploaded_by_username: Option<&str>,
    uploaded_by_legacy_name: Option<&str>,
    name: &str,
    content_type: &str,
    width: u32,
    height: u32,
    byte_size: usize,
    created_at: DateTime<Utc>,
) -> LogoView {
    LogoView {
        logo_id,
        destination,
        uploaded_by,
        uploaded_by_username: uploaded_by_username.map(str::to_owned),
        uploaded_by_legacy_name: uploaded_by_legacy_name.map(str::to_owned),
        name: name.to_owned(),
        content_type: content_type.to_owned(),
        width,
        height,
        byte_size: u64::try_from(byte_size).unwrap_or(u64::MAX),
        created_at,
    }
}

/// Look up a user's display fields for view-building. Mirrors the helper in
/// [`crate::routes::notecards`].
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

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::content_type_for_format;
    use crate::error::Error;

    #[test]
    fn accepted_formats_map_to_mime_types() {
        assert_eq!(
            content_type_for_format(image::ImageFormat::Png).ok(),
            Some("image/png")
        );
        assert_eq!(
            content_type_for_format(image::ImageFormat::Jpeg).ok(),
            Some("image/jpeg")
        );
        assert_eq!(
            content_type_for_format(image::ImageFormat::WebP).ok(),
            Some("image/webp")
        );
    }

    #[test]
    fn other_formats_are_rejected() {
        assert!(matches!(
            content_type_for_format(image::ImageFormat::Gif),
            Err(Error::BadRequest(_))
        ));
        assert!(matches!(
            content_type_for_format(image::ImageFormat::Bmp),
            Err(Error::BadRequest(_))
        ));
    }
}
