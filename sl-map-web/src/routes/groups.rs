//! HTTP handlers for groups.

use std::sync::atomic::Ordering;

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode as ReqwestStatusCode;
use axum::response::{IntoResponse as _, Response};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::auth::CurrentUser;
use crate::error::Error;
use crate::groups::{self, GroupMemberView, GroupRole, GroupView};
use crate::state::AppState;

/// Body for `POST /api/groups`.
#[derive(Debug, Deserialize)]
pub struct CreateGroupRequest {
    /// the human-readable group name.
    pub name: String,
}

/// Body for `PATCH /api/groups/{id}` (rename).
#[derive(Debug, Deserialize)]
pub struct RenameGroupRequest {
    /// the new group name.
    pub name: String,
}

/// Response carrying a single group view.
#[derive(Debug, Serialize)]
pub struct GroupResponse {
    /// the group.
    pub group: GroupView,
}

/// Response carrying the list of the current user's groups.
#[derive(Debug, Serialize)]
pub struct ListGroupsResponse {
    /// the groups, sorted by name.
    pub groups: Vec<GroupView>,
}

/// Response carrying the member list of a group.
#[derive(Debug, Serialize)]
pub struct ListMembersResponse {
    /// the members.
    pub members: Vec<GroupMemberView>,
}

/// Body for `PATCH /api/groups/{id}/members/{user_id}` (promote/demote).
#[derive(Debug, Deserialize)]
pub struct SetRoleRequest {
    /// the new role to assign.
    pub role: String,
}

/// `GET /api/groups` — list the calling user's groups.
///
/// # Errors
///
/// Returns [`Error::Database`] on lookup failure.
pub async fn list_mine(
    user: CurrentUser,
    State(state): State<AppState>,
) -> Result<Json<ListGroupsResponse>, Error> {
    let groups = groups::list_for_user(&state.db, user.user_id).await?;
    Ok(Json(ListGroupsResponse { groups }))
}

/// `POST /api/groups` — create a new group. The caller becomes its first
/// owner.
///
/// # Errors
///
/// Returns [`Error::BadRequest`] for an empty name, or [`Error::Database`]
/// on insert failure.
pub async fn create(
    user: CurrentUser,
    State(state): State<AppState>,
    Json(req): Json<CreateGroupRequest>,
) -> Result<(ReqwestStatusCode, Json<GroupResponse>), Error> {
    let name = req.name.trim();
    if name.is_empty() {
        return Err(Error::BadRequest("group name must not be empty".to_owned()));
    }
    let group_id = groups::create_group(&state.db, name, user.user_id).await?;
    let group = groups::get_for_user(&state.db, group_id, user.user_id).await?;
    Ok((ReqwestStatusCode::CREATED, Json(GroupResponse { group })))
}

/// `GET /api/groups/{id}` — get a single group.
///
/// # Errors
///
/// Returns [`Error::NotFound`] if the group does not exist or the caller is
/// not a member.
pub async fn get(
    user: CurrentUser,
    State(state): State<AppState>,
    Path(group_id): Path<Uuid>,
) -> Result<Json<GroupResponse>, Error> {
    let group = groups::get_for_user(&state.db, group_id, user.user_id).await?;
    Ok(Json(GroupResponse { group }))
}

/// `PATCH /api/groups/{id}` — rename the group. Owners only.
///
/// # Errors
///
/// Returns [`Error::Forbidden`] if the caller is not an owner.
pub async fn rename(
    user: CurrentUser,
    State(state): State<AppState>,
    Path(group_id): Path<Uuid>,
    Json(req): Json<RenameGroupRequest>,
) -> Result<Json<GroupResponse>, Error> {
    require_owner(&state, group_id, user.user_id).await?;
    let name = req.name.trim();
    if name.is_empty() {
        return Err(Error::BadRequest("group name must not be empty".to_owned()));
    }
    groups::rename_group(&state.db, group_id, name).await?;
    let group = groups::get_for_user(&state.db, group_id, user.user_id).await?;
    Ok(Json(GroupResponse { group }))
}

/// `DELETE /api/groups/{id}` — delete the group. Owners only. Cascades nuke
/// memberships, pending invitations, group-owned notecards, and group-owned
/// renders; the orphan-sweeper dirty flag is raised so render files on disk
/// are reaped on the next tick.
///
/// # Errors
///
/// Returns [`Error::Forbidden`] if the caller is not an owner.
pub async fn delete(
    user: CurrentUser,
    State(state): State<AppState>,
    Path(group_id): Path<Uuid>,
) -> Result<Response, Error> {
    require_owner(&state, group_id, user.user_id).await?;
    groups::delete_group(&state.db, group_id).await?;
    state.library_cleanup_dirty.store(true, Ordering::Release);
    Ok((ReqwestStatusCode::NO_CONTENT, "").into_response())
}

/// `GET /api/groups/{id}/members` — list members. Any group member.
///
/// # Errors
///
/// Returns [`Error::NotFound`] if the group does not exist or the caller is
/// not a member.
pub async fn list_members(
    user: CurrentUser,
    State(state): State<AppState>,
    Path(group_id): Path<Uuid>,
) -> Result<Json<ListMembersResponse>, Error> {
    require_member(&state, group_id, user.user_id).await?;
    let members = groups::list_members(&state.db, group_id).await?;
    Ok(Json(ListMembersResponse { members }))
}

/// `PATCH /api/groups/{id}/members/{user_id}` — change a member's role.
/// Owners only. Demotion that would leave zero owners is rejected.
///
/// # Errors
///
/// Returns [`Error::Forbidden`] if the caller is not an owner,
/// [`Error::BadRequest`] for a malformed role or a demotion that would
/// orphan the group, or [`Error::NotFound`] if the target is not a member.
pub async fn set_member_role(
    user: CurrentUser,
    State(state): State<AppState>,
    Path((group_id, target_user_id)): Path<(Uuid, Uuid)>,
    Json(req): Json<SetRoleRequest>,
) -> Result<Response, Error> {
    require_owner(&state, group_id, user.user_id).await?;
    let role = GroupRole::parse(req.role.trim())?;
    if role == GroupRole::Member {
        let current = groups::lookup_role(&state.db, group_id, target_user_id).await?;
        if current == Some(GroupRole::Owner) {
            let owner_count = groups::count_owners(&state.db, group_id).await?;
            if owner_count <= 1 {
                return Err(Error::BadRequest(
                    "cannot demote the last owner; promote another member first or delete the group"
                        .to_owned(),
                ));
            }
        }
    }
    groups::set_role(&state.db, group_id, target_user_id, role).await?;
    Ok((ReqwestStatusCode::NO_CONTENT, "").into_response())
}

/// `DELETE /api/groups/{id}/members/{user_id}` — remove a member. Owners
/// only. Removing the last owner is rejected with a friendly message.
///
/// # Errors
///
/// As above plus [`Error::NotFound`] if the target is not a member.
pub async fn remove_member(
    user: CurrentUser,
    State(state): State<AppState>,
    Path((group_id, target_user_id)): Path<(Uuid, Uuid)>,
) -> Result<Response, Error> {
    require_owner(&state, group_id, user.user_id).await?;
    let target_role = groups::lookup_role(&state.db, group_id, target_user_id).await?;
    if target_role == Some(GroupRole::Owner) {
        let owner_count = groups::count_owners(&state.db, group_id).await?;
        if owner_count <= 1 {
            return Err(Error::BadRequest(
                "cannot remove the last owner; promote another member first or delete the group"
                    .to_owned(),
            ));
        }
    }
    groups::remove_member(&state.db, group_id, target_user_id).await?;
    Ok((ReqwestStatusCode::NO_CONTENT, "").into_response())
}

/// `POST /api/groups/{id}/leave` — the calling user leaves the group. The
/// last owner cannot leave; they must delete the group instead.
///
/// # Errors
///
/// Returns [`Error::BadRequest`] if the caller is the sole owner.
pub async fn leave(
    user: CurrentUser,
    State(state): State<AppState>,
    Path(group_id): Path<Uuid>,
) -> Result<Response, Error> {
    let role = groups::lookup_role(&state.db, group_id, user.user_id).await?;
    let Some(role) = role else {
        return Err(Error::NotFound(format!(
            "you are not a member of group {group_id}"
        )));
    };
    if role == GroupRole::Owner {
        let owner_count = groups::count_owners(&state.db, group_id).await?;
        if owner_count <= 1 {
            return Err(Error::BadRequest(
                "you are the last owner of this group; promote another member to owner or \
                 delete the group instead of leaving"
                    .to_owned(),
            ));
        }
    }
    groups::remove_member(&state.db, group_id, user.user_id).await?;
    Ok((ReqwestStatusCode::NO_CONTENT, "").into_response())
}

/// Require that the calling user is an owner of the given group.
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

/// Require that the calling user is at least a member of the given group.
async fn require_member(state: &AppState, group_id: Uuid, user_id: Uuid) -> Result<(), Error> {
    groups::require_exists(&state.db, group_id).await?;
    if groups::lookup_role(&state.db, group_id, user_id)
        .await?
        .is_some()
    {
        Ok(())
    } else {
        Err(Error::Forbidden(format!(
            "not a member of group {group_id}"
        )))
    }
}
