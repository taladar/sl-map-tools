//! HTTP handlers for group invitations.

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode as ReqwestStatusCode;
use axum::response::{IntoResponse as _, Response};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::auth::{self, CurrentUser, uuid_from_bytes};
use crate::error::Error;
use crate::groups::{self, GroupRole, InvitationStatus};
use crate::state::AppState;

/// Body for `POST /api/groups/{id}/invitations`.
#[derive(Debug, Deserialize)]
pub struct CreateInvitationRequest {
    /// Avatar UUID, username, or legacy name to invite.
    pub identifier: String,
    /// Role the invitee will be granted on acceptance. Defaults to
    /// `"member"`.
    #[serde(default = "default_target_role")]
    pub target_role: String,
}

/// Default target role when the field is omitted.
fn default_target_role() -> String {
    "member".to_owned()
}

/// View of one invitation as returned to either the inviter (the group's
/// owner list) or the invitee (their pending list).
#[derive(Debug, Clone, Serialize)]
pub struct InvitationView {
    /// the invitation id.
    pub invitation_id: Uuid,
    /// the target group.
    pub group_id: Uuid,
    /// the group's display name (for the invitee's UI).
    pub group_name: String,
    /// the inviter's user id.
    pub inviter_id: Uuid,
    /// the inviter's username.
    pub inviter_username: String,
    /// the inviter's legacy name.
    pub inviter_legacy_name: String,
    /// the invitee's user id.
    pub invitee_id: Uuid,
    /// the invitee's username.
    pub invitee_username: String,
    /// the invitee's legacy name.
    pub invitee_legacy_name: String,
    /// the role the invitee will receive on accept.
    pub target_role: GroupRole,
    /// the current invitation status.
    pub status: InvitationStatus,
    /// when the invitation was created.
    pub created_at: DateTime<Utc>,
    /// when the invitation reached a terminal status, if it has.
    pub responded_at: Option<DateTime<Utc>>,
}

/// Response carrying a list of invitations.
#[derive(Debug, Serialize)]
pub struct ListInvitationsResponse {
    /// the invitations.
    pub invitations: Vec<InvitationView>,
}

/// Response carrying a single invitation.
#[derive(Debug, Serialize)]
pub struct InvitationResponse {
    /// the invitation.
    pub invitation: InvitationView,
}

/// `POST /api/groups/{id}/invitations` — create an invitation. Owners only.
///
/// # Errors
///
/// Returns [`Error::Forbidden`] if the caller is not an owner;
/// [`Error::BadRequest`] for an unknown identifier (same message as login to
/// avoid user enumeration), an invalid target role, an attempt to invite
/// the caller themselves, or if a pending invitation for the same target
/// already exists.
pub async fn create(
    user: CurrentUser,
    State(state): State<AppState>,
    Path(group_id): Path<Uuid>,
    Json(req): Json<CreateInvitationRequest>,
) -> Result<(ReqwestStatusCode, Json<InvitationResponse>), Error> {
    require_owner(&state, group_id, user.user_id).await?;
    let identifier = req.identifier.trim();
    if identifier.is_empty() {
        return Err(Error::BadRequest("identifier is required".to_owned()));
    }
    let target_role = GroupRole::parse(req.target_role.trim())?;

    let invitee_row = auth::lookup_user_by_identifier(&state.db, identifier)
        .await?
        .ok_or_else(|| {
            // identical to the login flow's "invalid identifier" to avoid
            // letting an outsider enumerate the user table via this endpoint.
            Error::BadRequest("no user matches that identifier".to_owned())
        })?;
    let (invitee_bytes, invitee_legacy, invitee_username, _password_hash) = invitee_row;
    let invitee_id = uuid_from_bytes(&invitee_bytes).ok_or_else(|| {
        tracing::error!("invitee user_id blob was the wrong length");
        Error::Database
    })?;

    if invitee_id == user.user_id {
        return Err(Error::BadRequest(
            "cannot invite yourself to a group you are already in".to_owned(),
        ));
    }
    if let Some(existing_role) = groups::lookup_role(&state.db, group_id, invitee_id).await? {
        return Err(Error::BadRequest(format!(
            "user is already a {} of this group",
            existing_role.as_str()
        )));
    }

    let invitation_id = Uuid::new_v4();
    let now = Utc::now();
    let insert = sqlx::query(
        "INSERT INTO group_invitations \
            (invitation_id, group_id, invitee_id, inviter_id, target_role, status, created_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, 'pending', ?6)",
    )
    .bind(invitation_id.as_bytes().to_vec())
    .bind(group_id.as_bytes().to_vec())
    .bind(invitee_id.as_bytes().to_vec())
    .bind(user.user_id.as_bytes().to_vec())
    .bind(target_role.as_str())
    .bind(now)
    .execute(&state.db)
    .await;
    if let Err(err) = insert {
        // unique partial index on (group_id, invitee_id) WHERE status='pending'
        if let sqlx::Error::Database(db_err) = &err
            && db_err.is_unique_violation()
        {
            return Err(Error::BadRequest(
                "there is already a pending invitation for this user".to_owned(),
            ));
        }
        tracing::error!("invitation insert failed: {err}");
        return Err(Error::Database);
    }

    let inviter_username = user.username.clone();
    let inviter_legacy_name = user.legacy_name.clone();
    let group_name = group_name(&state, group_id).await?;
    let view = InvitationView {
        invitation_id,
        group_id,
        group_name,
        inviter_id: user.user_id,
        inviter_username,
        inviter_legacy_name,
        invitee_id,
        invitee_username,
        invitee_legacy_name: invitee_legacy,
        target_role,
        status: InvitationStatus::Pending,
        created_at: now,
        responded_at: None,
    };
    Ok((
        ReqwestStatusCode::CREATED,
        Json(InvitationResponse { invitation: view }),
    ))
}

/// `GET /api/groups/{id}/invitations` — list this group's invitations.
/// Owners only.
///
/// # Errors
///
/// Returns [`Error::Forbidden`] if the caller is not an owner.
pub async fn list_for_group(
    user: CurrentUser,
    State(state): State<AppState>,
    Path(group_id): Path<Uuid>,
) -> Result<Json<ListInvitationsResponse>, Error> {
    require_owner(&state, group_id, user.user_id).await?;
    let invitations = fetch_invitations(
        &state,
        "WHERE group_invitations.group_id = ?1 ORDER BY group_invitations.created_at DESC",
        Some(group_id.as_bytes().to_vec()),
    )
    .await?;
    Ok(Json(ListInvitationsResponse { invitations }))
}

/// `GET /api/invitations` — list pending invitations addressed to the
/// calling user.
///
/// # Errors
///
/// Returns [`Error::Database`] on lookup failure.
pub async fn list_mine(
    user: CurrentUser,
    State(state): State<AppState>,
) -> Result<Json<ListInvitationsResponse>, Error> {
    let invitations = fetch_invitations(
        &state,
        "WHERE group_invitations.invitee_id = ?1 AND group_invitations.status = 'pending' \
         ORDER BY group_invitations.created_at DESC",
        Some(user.user_id.as_bytes().to_vec()),
    )
    .await?;
    Ok(Json(ListInvitationsResponse { invitations }))
}

/// `POST /api/invitations/{id}/accept` — accept an invitation. Only the
/// invitee may accept.
///
/// # Errors
///
/// Returns [`Error::Forbidden`] if the caller is not the invitee.
pub async fn accept(
    user: CurrentUser,
    State(state): State<AppState>,
    Path(invitation_id): Path<Uuid>,
) -> Result<Response, Error> {
    let row = fetch_pending_for_invitee(&state, invitation_id, user.user_id).await?;
    let (group_id_bytes, target_role) = row;
    let now = Utc::now();
    let mut tx = state.db.begin().await.map_err(|err| {
        tracing::error!("begin accept tx failed: {err}");
        Error::Database
    })?;
    sqlx::query(
        "INSERT INTO group_memberships (group_id, user_id, role, created_at) \
         VALUES (?1, ?2, ?3, ?4) \
         ON CONFLICT(group_id, user_id) DO UPDATE SET role = excluded.role",
    )
    .bind(&group_id_bytes)
    .bind(user.user_id.as_bytes().to_vec())
    .bind(target_role.as_str())
    .bind(now)
    .execute(&mut *tx)
    .await
    .map_err(|err| {
        tracing::error!("insert membership on accept failed: {err}");
        Error::Database
    })?;
    sqlx::query(
        "UPDATE group_invitations SET status = 'accepted', responded_at = ?1 \
         WHERE invitation_id = ?2",
    )
    .bind(now)
    .bind(invitation_id.as_bytes().to_vec())
    .execute(&mut *tx)
    .await
    .map_err(|err| {
        tracing::error!("update invitation on accept failed: {err}");
        Error::Database
    })?;
    tx.commit().await.map_err(|err| {
        tracing::error!("commit accept tx failed: {err}");
        Error::Database
    })?;
    Ok((ReqwestStatusCode::NO_CONTENT, "").into_response())
}

/// `POST /api/invitations/{id}/reject` — reject an invitation. Only the
/// invitee may reject.
///
/// # Errors
///
/// Returns [`Error::Forbidden`] if the caller is not the invitee.
pub async fn reject(
    user: CurrentUser,
    State(state): State<AppState>,
    Path(invitation_id): Path<Uuid>,
) -> Result<Response, Error> {
    let _row = fetch_pending_for_invitee(&state, invitation_id, user.user_id).await?;
    let now = Utc::now();
    sqlx::query(
        "UPDATE group_invitations SET status = 'rejected', responded_at = ?1 \
         WHERE invitation_id = ?2",
    )
    .bind(now)
    .bind(invitation_id.as_bytes().to_vec())
    .execute(&state.db)
    .await
    .map_err(|err| {
        tracing::error!("update invitation on reject failed: {err}");
        Error::Database
    })?;
    Ok((ReqwestStatusCode::NO_CONTENT, "").into_response())
}

/// Fetch the pending invitation row for the calling invitee. Returns
/// `(group_id_bytes, target_role)`. Returns [`Error::NotFound`] if missing
/// or [`Error::Forbidden`] if the caller is not the invitee.
async fn fetch_pending_for_invitee(
    state: &AppState,
    invitation_id: Uuid,
    invitee_id: Uuid,
) -> Result<(Vec<u8>, GroupRole), Error> {
    let row: Option<(Vec<u8>, Vec<u8>, String, String)> = sqlx::query_as(
        "SELECT group_id, invitee_id, target_role, status FROM group_invitations \
         WHERE invitation_id = ?1",
    )
    .bind(invitation_id.as_bytes().to_vec())
    .fetch_optional(&state.db)
    .await
    .map_err(|err| {
        tracing::error!("fetch invitation failed: {err}");
        Error::Database
    })?;
    let (group_id_bytes, row_invitee_bytes, target_role, status) =
        row.ok_or_else(|| Error::NotFound(format!("invitation {invitation_id}")))?;
    let row_invitee = uuid_from_bytes(&row_invitee_bytes).ok_or_else(|| {
        tracing::error!("bad invitee uuid in invitation");
        Error::Database
    })?;
    if row_invitee != invitee_id {
        return Err(Error::Forbidden(
            "only the invitee may accept or reject an invitation".to_owned(),
        ));
    }
    if status != "pending" {
        return Err(Error::BadRequest(format!("invitation is already {status}")));
    }
    let target_role = GroupRole::parse(&target_role)?;
    Ok((group_id_bytes, target_role))
}

/// Run a parameterised lookup of invitations joined with group + user info.
/// The `where_clause` may contain a single `?1` placeholder that will be
/// bound to `bind_blob`.
async fn fetch_invitations(
    state: &AppState,
    where_clause: &str,
    bind_blob: Option<Vec<u8>>,
) -> Result<Vec<InvitationView>, Error> {
    let sql = format!(
        "SELECT group_invitations.invitation_id, \
                group_invitations.group_id, \
                groups.name, \
                group_invitations.inviter_id, \
                inviter.username, inviter.legacy_name, \
                group_invitations.invitee_id, \
                invitee.username, invitee.legacy_name, \
                group_invitations.target_role, \
                group_invitations.status, \
                group_invitations.created_at, \
                group_invitations.responded_at \
         FROM group_invitations \
         JOIN groups ON groups.group_id = group_invitations.group_id \
         JOIN users AS inviter ON inviter.user_id = group_invitations.inviter_id \
         JOIN users AS invitee ON invitee.user_id = group_invitations.invitee_id \
         {where_clause}"
    );
    let mut query = sqlx::query_as::<
        _,
        (
            Vec<u8>,
            Vec<u8>,
            String,
            Vec<u8>,
            String,
            String,
            Vec<u8>,
            String,
            String,
            String,
            String,
            DateTime<Utc>,
            Option<DateTime<Utc>>,
        ),
    >(&sql);
    if let Some(blob) = bind_blob {
        query = query.bind(blob);
    }
    let rows = query.fetch_all(&state.db).await.map_err(|err| {
        tracing::error!("fetch invitations failed: {err}");
        Error::Database
    })?;
    let mut out = Vec::with_capacity(rows.len());
    for (
        invitation_bytes,
        group_id_bytes,
        group_name,
        inviter_bytes,
        inviter_username,
        inviter_legacy_name,
        invitee_bytes,
        invitee_username,
        invitee_legacy_name,
        target_role,
        status,
        created_at,
        responded_at,
    ) in rows
    {
        let invitation_id = uuid_from_bytes(&invitation_bytes).ok_or_else(|| {
            tracing::error!("bad invitation uuid");
            Error::Database
        })?;
        let group_id = uuid_from_bytes(&group_id_bytes).ok_or_else(|| {
            tracing::error!("bad group uuid");
            Error::Database
        })?;
        let inviter_id = uuid_from_bytes(&inviter_bytes).ok_or_else(|| {
            tracing::error!("bad inviter uuid");
            Error::Database
        })?;
        let invitee_id = uuid_from_bytes(&invitee_bytes).ok_or_else(|| {
            tracing::error!("bad invitee uuid");
            Error::Database
        })?;
        let status = match status.as_str() {
            "pending" => InvitationStatus::Pending,
            "accepted" => InvitationStatus::Accepted,
            "rejected" => InvitationStatus::Rejected,
            other => {
                return Err(Error::BadRequest(format!(
                    "unknown invitation status `{other}`"
                )));
            }
        };
        out.push(InvitationView {
            invitation_id,
            group_id,
            group_name,
            inviter_id,
            inviter_username,
            inviter_legacy_name,
            invitee_id,
            invitee_username,
            invitee_legacy_name,
            target_role: GroupRole::parse(&target_role)?,
            status,
            created_at,
            responded_at,
        });
    }
    Ok(out)
}

/// Fetch a group's name (used in invitation views).
async fn group_name(state: &AppState, group_id: Uuid) -> Result<String, Error> {
    let row: Option<(String,)> = sqlx::query_as("SELECT name FROM groups WHERE group_id = ?1")
        .bind(group_id.as_bytes().to_vec())
        .fetch_optional(&state.db)
        .await
        .map_err(|err| {
            tracing::error!("group name lookup failed: {err}");
            Error::Database
        })?;
    row.map(|(n,)| n)
        .ok_or_else(|| Error::NotFound(format!("group {group_id}")))
}

/// Ensure the calling user is an owner of the group.
async fn require_owner(state: &AppState, group_id: Uuid, user_id: Uuid) -> Result<(), Error> {
    groups::require_exists(&state.db, group_id).await?;
    if groups::lookup_role(&state.db, group_id, user_id).await? == Some(GroupRole::Owner) {
        Ok(())
    } else {
        Err(Error::Forbidden(format!(
            "must be an owner of group {group_id}"
        )))
    }
}
