//! Saved notecards and saved renders: scope/destination types, permission
//! helpers, and the orphan-file sweeper.
//!
//! Every handler that mutates or reads a saved item funnels through one of
//! the `assert_can_*` helpers here so the permission rules live in exactly
//! one place.

use std::collections::HashSet;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::SqlitePool;
use tokio::time::interval;
use uuid::Uuid;

use crate::auth::uuid_from_bytes;
use crate::error::Error;
use crate::groups::{self, GroupRole};
use crate::storage;

/// Owner scope of a saved notecard or saved render. Exactly one of the two
/// variants is set; the schema CHECK constraint mirrors this at the DB.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Destination {
    /// Owned by a single user (the user's personal library).
    Personal,
    /// Owned by a group.
    Group {
        /// the owning group's id.
        group_id: Uuid,
    },
}

impl Destination {
    /// Parse a destination from a form / query string. Accepted values:
    /// `"personal"` or `"group:<uuid>"`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::BadRequest`] for any other shape.
    pub fn parse(raw: &str) -> Result<Self, Error> {
        let trimmed = raw.trim();
        if trimmed.eq_ignore_ascii_case("personal") {
            return Ok(Self::Personal);
        }
        if let Some(rest) = trimmed
            .strip_prefix("group:")
            .or_else(|| trimmed.strip_prefix("Group:"))
        {
            let group_id = Uuid::parse_str(rest.trim()).map_err(|err| {
                Error::BadRequest(format!("invalid group uuid in destination: {err}"))
            })?;
            return Ok(Self::Group { group_id });
        }
        Err(Error::BadRequest(format!(
            "destination must be `personal` or `group:<uuid>`, got `{trimmed}`"
        )))
    }

    /// Round-trip a destination back to the string form. Useful for hidden
    /// form fields and links.
    #[must_use]
    pub fn render_string(self) -> String {
        match self {
            Self::Personal => "personal".to_owned(),
            Self::Group { group_id } => format!("group:{group_id}"),
        }
    }
}

/// Public, serializable record of a saved notecard.
#[derive(Debug, Clone, Serialize)]
pub struct NotecardView {
    /// the notecard's identifier.
    pub notecard_id: Uuid,
    /// the destination the notecard belongs to.
    pub destination: Destination,
    /// the avatar that uploaded the notecard.
    pub uploaded_by: Uuid,
    /// the avatar's username, for display.
    pub uploaded_by_username: String,
    /// the avatar's legacy name, for display.
    pub uploaded_by_legacy_name: String,
    /// the human-supplied display name of the notecard.
    pub name: String,
    /// when the notecard was saved.
    pub created_at: DateTime<Utc>,
}

/// Public, serializable record of a saved render.
#[derive(Debug, Clone, Serialize)]
pub struct RenderView {
    /// the render's identifier.
    pub render_id: Uuid,
    /// the destination the render belongs to.
    pub destination: Destination,
    /// the avatar that started the render.
    pub created_by: Uuid,
    /// the avatar's username, for display.
    pub created_by_username: String,
    /// the avatar's legacy name, for display.
    pub created_by_legacy_name: String,
    /// the linked saved notecard, if any (USB-notecard renders only).
    pub notecard_id: Option<Uuid>,
    /// what the render was launched from.
    pub kind: String,
    /// current status: `in_progress`, `done`, or `failed`.
    pub status: String,
    /// error message if `status == "failed"`.
    pub error_message: Option<String>,
    /// when the row was created (submit time).
    pub created_at: DateTime<Utc>,
    /// when the row reached a terminal state.
    pub finished_at: Option<DateTime<Utc>>,
    /// whether a without-route variant is available for download.
    pub has_without_route: bool,
    /// the content type of the stored image.
    pub content_type: Option<String>,
}

/// Verify that the calling user is allowed to *write* to the given
/// destination. Personal scope is always allowed; group scope requires
/// owner membership.
///
/// # Errors
///
/// Returns [`Error::Forbidden`] if the user is not an owner of the target
/// group, [`Error::NotFound`] if the group does not exist.
pub async fn assert_can_write(
    db: &SqlitePool,
    current_user: Uuid,
    destination: Destination,
) -> Result<(), Error> {
    match destination {
        Destination::Personal => Ok(()),
        Destination::Group { group_id } => {
            groups::require_exists(db, group_id).await?;
            let role = groups::lookup_role(db, group_id, current_user).await?;
            if role == Some(GroupRole::Owner) {
                Ok(())
            } else {
                Err(Error::Forbidden(format!(
                    "must be an owner of group {group_id} to save items there"
                )))
            }
        }
    }
}

/// Resolve a destination to whether the current user can *view* its
/// contents and (for groups) what role they have. Personal scope means the
/// user is the owner; otherwise [`Error::Forbidden`] is returned.
///
/// # Errors
///
/// Returns [`Error::Forbidden`] if the user is not a member of the target
/// group.
pub async fn assert_can_view(
    db: &SqlitePool,
    current_user: Uuid,
    destination: Destination,
) -> Result<Option<GroupRole>, Error> {
    match destination {
        Destination::Personal => Ok(None),
        Destination::Group { group_id } => {
            groups::require_exists(db, group_id).await?;
            let role = groups::lookup_role(db, group_id, current_user).await?;
            role.map_or_else(
                || {
                    Err(Error::Forbidden(format!(
                        "not a member of group {group_id}"
                    )))
                },
                |r| Ok(Some(r)),
            )
        }
    }
}

/// Convert a `(owner_user_id, owner_group_id)` row pair (exactly one of the
/// two is Some) into a [`Destination`].
///
/// # Errors
///
/// Returns [`Error::Database`] if both or neither are set (the DB CHECK
/// constraint should prevent this, but we still surface a clear error).
pub fn destination_from_columns(
    owner_user_id: Option<Vec<u8>>,
    owner_group_id: Option<Vec<u8>>,
) -> Result<Destination, Error> {
    match (owner_user_id, owner_group_id) {
        (Some(_), None) => Ok(Destination::Personal),
        (None, Some(gid_bytes)) => {
            let group_id = uuid_from_bytes(&gid_bytes).ok_or_else(|| {
                tracing::error!("bad group uuid blob in destination column");
                Error::Database
            })?;
            Ok(Destination::Group { group_id })
        }
        _ => {
            tracing::error!("saved row had both or neither owner column set");
            Err(Error::Database)
        }
    }
}

/// Permission gate for reading a notecard. Personal: must be the owner.
/// Group: must be a member.
///
/// # Errors
///
/// Returns [`Error::Forbidden`] if the user is not allowed to view the
/// notecard; [`Error::NotFound`] if the notecard does not exist.
pub async fn assert_can_read_notecard(
    db: &SqlitePool,
    current_user: Uuid,
    notecard_id: Uuid,
) -> Result<NotecardRow, Error> {
    let row = fetch_notecard_row(db, notecard_id).await?;
    let destination =
        destination_from_columns(row.owner_user_id.clone(), row.owner_group_id.clone())?;
    match destination {
        Destination::Personal => {
            let owner = row.owner_user_id.as_deref().and_then(uuid_from_bytes);
            if owner == Some(current_user) {
                Ok(row)
            } else {
                Err(Error::Forbidden(format!(
                    "not allowed to view notecard {notecard_id}"
                )))
            }
        }
        Destination::Group { group_id } => {
            if groups::lookup_role(db, group_id, current_user)
                .await?
                .is_some()
            {
                Ok(row)
            } else {
                Err(Error::Forbidden(format!(
                    "not a member of group {group_id}"
                )))
            }
        }
    }
}

/// Permission gate for reading a render. Personal: must be the owner.
/// Group: must be a member; members may not see `in_progress` or `failed`
/// renders.
///
/// # Errors
///
/// Returns [`Error::Forbidden`] if the user is not allowed to view the
/// render; [`Error::NotFound`] if the render does not exist.
pub async fn assert_can_read_render(
    db: &SqlitePool,
    current_user: Uuid,
    render_id: Uuid,
) -> Result<RenderRow, Error> {
    let row = fetch_render_row(db, render_id).await?;
    let destination =
        destination_from_columns(row.owner_user_id.clone(), row.owner_group_id.clone())?;
    match destination {
        Destination::Personal => {
            let owner = row.owner_user_id.as_deref().and_then(uuid_from_bytes);
            if owner == Some(current_user) {
                Ok(row)
            } else {
                Err(Error::Forbidden(format!(
                    "not allowed to view render {render_id}"
                )))
            }
        }
        Destination::Group { group_id } => {
            let role = groups::lookup_role(db, group_id, current_user).await?;
            match role {
                Some(GroupRole::Owner) => Ok(row),
                Some(GroupRole::Member) => {
                    if row.status == "done" {
                        Ok(row)
                    } else {
                        Err(Error::Forbidden(
                            "members may only view finished renders in a group library".to_owned(),
                        ))
                    }
                }
                None => Err(Error::Forbidden(format!(
                    "not a member of group {group_id}"
                ))),
            }
        }
    }
}

/// Permission gate for deleting a render. Personal: must be the owner.
/// Group: must be an owner of the group.
///
/// # Errors
///
/// Returns [`Error::Forbidden`] if the user lacks delete permission;
/// [`Error::NotFound`] if the render does not exist.
pub async fn assert_can_delete_render(
    db: &SqlitePool,
    current_user: Uuid,
    render_id: Uuid,
) -> Result<RenderRow, Error> {
    let row = fetch_render_row(db, render_id).await?;
    let destination =
        destination_from_columns(row.owner_user_id.clone(), row.owner_group_id.clone())?;
    match destination {
        Destination::Personal => {
            let owner = row.owner_user_id.as_deref().and_then(uuid_from_bytes);
            if owner == Some(current_user) {
                Ok(row)
            } else {
                Err(Error::Forbidden(format!(
                    "not allowed to delete render {render_id}"
                )))
            }
        }
        Destination::Group { group_id } => {
            if groups::lookup_role(db, group_id, current_user).await? == Some(GroupRole::Owner) {
                Ok(row)
            } else {
                Err(Error::Forbidden(
                    "must be a group owner to delete a group render".to_owned(),
                ))
            }
        }
    }
}

/// Permission gate for deleting a notecard (same rule as renders).
///
/// # Errors
///
/// Returns [`Error::Forbidden`] if the user lacks delete permission;
/// [`Error::NotFound`] if the notecard does not exist.
pub async fn assert_can_delete_notecard(
    db: &SqlitePool,
    current_user: Uuid,
    notecard_id: Uuid,
) -> Result<NotecardRow, Error> {
    let row = fetch_notecard_row(db, notecard_id).await?;
    let destination =
        destination_from_columns(row.owner_user_id.clone(), row.owner_group_id.clone())?;
    match destination {
        Destination::Personal => {
            let owner = row.owner_user_id.as_deref().and_then(uuid_from_bytes);
            if owner == Some(current_user) {
                Ok(row)
            } else {
                Err(Error::Forbidden(format!(
                    "not allowed to delete notecard {notecard_id}"
                )))
            }
        }
        Destination::Group { group_id } => {
            if groups::lookup_role(db, group_id, current_user).await? == Some(GroupRole::Owner) {
                Ok(row)
            } else {
                Err(Error::Forbidden(
                    "must be a group owner to delete a group notecard".to_owned(),
                ))
            }
        }
    }
}

/// Raw row fields for a saved notecard as fetched from the DB.
#[derive(Debug, Clone)]
pub struct NotecardRow {
    /// the notecard id.
    pub notecard_id: Uuid,
    /// raw bytes of the personal owner column, if any.
    pub owner_user_id: Option<Vec<u8>>,
    /// raw bytes of the group owner column, if any.
    pub owner_group_id: Option<Vec<u8>>,
    /// the uploading avatar id.
    pub uploaded_by: Uuid,
    /// the notecard's display name.
    pub name: String,
    /// the raw notecard body (the text the user uploaded).
    pub body: String,
    /// when the row was created.
    pub created_at: DateTime<Utc>,
}

/// Raw row fields for a saved render as fetched from the DB.
#[derive(Debug, Clone)]
pub struct RenderRow {
    /// the render id.
    pub render_id: Uuid,
    /// raw bytes of the personal owner column, if any.
    pub owner_user_id: Option<Vec<u8>>,
    /// raw bytes of the group owner column, if any.
    pub owner_group_id: Option<Vec<u8>>,
    /// the avatar that created the render.
    pub created_by: Uuid,
    /// the linked notecard id, if any.
    pub notecard_id: Option<Uuid>,
    /// the render kind: `grid_rectangle` or `usb_notecard`.
    pub kind: String,
    /// the current status.
    pub status: String,
    /// the error message if status == "failed".
    pub error_message: Option<String>,
    /// the settings JSON used to launch the render.
    pub settings_json: String,
    /// the metadata JSON produced by the render (if `done`).
    pub metadata_json: Option<String>,
    /// the content type of the stored image.
    pub content_type: Option<String>,
    /// the filename of the primary image file under `<storage_dir>/renders/`.
    pub image_filename: Option<String>,
    /// the filename of the without-route variant, if any.
    pub image_without_route_filename: Option<String>,
    /// when the row was created.
    pub created_at: DateTime<Utc>,
    /// when the row reached a terminal state.
    pub finished_at: Option<DateTime<Utc>>,
}

/// Tuple shape returned by the `saved_notecards` lookup query.
type NotecardRowTuple = (
    Option<Vec<u8>>,
    Option<Vec<u8>>,
    Vec<u8>,
    String,
    String,
    DateTime<Utc>,
);

/// Fetch a notecard row by id; returns [`Error::NotFound`] if missing.
async fn fetch_notecard_row(db: &SqlitePool, notecard_id: Uuid) -> Result<NotecardRow, Error> {
    let row: Option<NotecardRowTuple> = sqlx::query_as(
        "SELECT owner_user_id, owner_group_id, uploaded_by, name, body, created_at \
         FROM saved_notecards WHERE notecard_id = ?1",
    )
    .bind(notecard_id.as_bytes().to_vec())
    .fetch_optional(db)
    .await
    .map_err(|err| {
        tracing::error!("notecard fetch failed: {err}");
        Error::Database
    })?;
    let (owner_user_id, owner_group_id, uploaded_by_bytes, name, body, created_at) =
        row.ok_or_else(|| Error::NotFound(format!("notecard {notecard_id}")))?;
    let uploaded_by = uuid_from_bytes(&uploaded_by_bytes).ok_or_else(|| {
        tracing::error!("bad uploaded_by uuid in saved_notecards");
        Error::Database
    })?;
    Ok(NotecardRow {
        notecard_id,
        owner_user_id,
        owner_group_id,
        uploaded_by,
        name,
        body,
        created_at,
    })
}

/// Tuple shape returned by the `saved_renders` lookup query.
type RenderRowTuple = (
    Option<Vec<u8>>,
    Option<Vec<u8>>,
    Vec<u8>,
    Option<Vec<u8>>,
    String,
    String,
    Option<String>,
    String,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    DateTime<Utc>,
    Option<DateTime<Utc>>,
);

/// Fetch a render row by id; returns [`Error::NotFound`] if missing.
async fn fetch_render_row(db: &SqlitePool, render_id: Uuid) -> Result<RenderRow, Error> {
    let row: Option<RenderRowTuple> = sqlx::query_as(
        "SELECT owner_user_id, owner_group_id, created_by, notecard_id, kind, status, \
                error_message, settings_json, metadata_json, content_type, \
                image_filename, image_without_route_filename, created_at, finished_at \
         FROM saved_renders WHERE render_id = ?1",
    )
    .bind(render_id.as_bytes().to_vec())
    .fetch_optional(db)
    .await
    .map_err(|err| {
        tracing::error!("render fetch failed: {err}");
        Error::Database
    })?;
    let (
        owner_user_id,
        owner_group_id,
        created_by_bytes,
        notecard_bytes,
        kind,
        status,
        error_message,
        settings_json,
        metadata_json,
        content_type,
        image_filename,
        image_without_route_filename,
        created_at,
        finished_at,
    ) = row.ok_or_else(|| Error::NotFound(format!("render {render_id}")))?;
    let created_by = uuid_from_bytes(&created_by_bytes).ok_or_else(|| {
        tracing::error!("bad created_by uuid in saved_renders");
        Error::Database
    })?;
    let notecard_id = notecard_bytes
        .as_deref()
        .map(uuid_from_bytes)
        .map(|opt| {
            opt.ok_or_else(|| {
                tracing::error!("bad notecard_id uuid in saved_renders");
                Error::Database
            })
        })
        .transpose()?;
    Ok(RenderRow {
        render_id,
        owner_user_id,
        owner_group_id,
        created_by,
        notecard_id,
        kind,
        status,
        error_message,
        settings_json,
        metadata_json,
        content_type,
        image_filename,
        image_without_route_filename,
        created_at,
        finished_at,
    })
}

/// Run the orphan-file sweeper. Wakes every `period` seconds; if the dirty
/// flag is unset, the tick is a cheap no-op. When the flag is set the
/// sweeper scans `<storage_dir>/renders/` and unlinks any file whose UUID is
/// not present in `saved_renders`. A scan failure re-raises the flag so the
/// next tick retries.
pub async fn run_orphan_sweeper(
    db: SqlitePool,
    storage_dir: Arc<Path>,
    dirty: Arc<AtomicBool>,
    period: Duration,
) {
    let mut tick = interval(period);
    loop {
        tick.tick().await;
        if !dirty.swap(false, Ordering::AcqRel) {
            tracing::debug!("orphan sweeper: no work flagged, skipping");
            continue;
        }
        match sweep_once(&db, storage_dir.as_ref()).await {
            Ok(count) => {
                if count > 0 {
                    tracing::info!("orphan sweeper: removed {count} stale render file(s)");
                }
            }
            Err(err) => {
                tracing::warn!("orphan sweeper run failed: {err}; will retry on next tick");
                dirty.store(true, Ordering::Release);
            }
        }
    }
}

/// One pass of the sweeper: list files, query live render ids, unlink the
/// difference. Returns the number of files unlinked.
async fn sweep_once(db: &SqlitePool, storage_dir: &Path) -> Result<usize, Error> {
    let files = storage::list_render_files(storage_dir)?;
    let live: Vec<Vec<u8>> = sqlx::query_scalar("SELECT render_id FROM saved_renders")
        .fetch_all(db)
        .await
        .map_err(|err| {
            tracing::error!("sweeper render id query failed: {err}");
            Error::Database
        })?;
    let live_set: HashSet<Uuid> = live
        .into_iter()
        .filter_map(|b| uuid_from_bytes(&b))
        .collect();
    let mut removed = 0_usize;
    for filename in files {
        let Some(id) = storage::parse_render_id_from_filename(&filename) else {
            continue;
        };
        if live_set.contains(&id) {
            continue;
        }
        if let Err(err) = storage::try_delete_render_file(storage_dir, &filename) {
            tracing::warn!("sweeper failed to unlink {filename}: {err}");
            continue;
        }
        removed = removed.saturating_add(1);
    }
    Ok(removed)
}
