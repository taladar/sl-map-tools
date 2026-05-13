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
/// Owners only. An owner may promote any member to owner, but the only
/// owner→member transition allowed is self-demotion, and only when at
/// least one other owner remains.
///
/// # Errors
///
/// Returns [`Error::Forbidden`] if the caller is not an owner or attempts
/// to demote a different owner, [`Error::BadRequest`] for a malformed role
/// or a self-demotion that would orphan the group, or [`Error::NotFound`]
/// if the target is not a member.
pub async fn set_member_role(
    user: CurrentUser,
    State(state): State<AppState>,
    Path((group_id, target_user_id)): Path<(Uuid, Uuid)>,
    Json(req): Json<SetRoleRequest>,
) -> Result<Response, Error> {
    require_owner(&state, group_id, user.user_id).await?;
    let role = GroupRole::parse(req.role.trim())?;
    let current = groups::lookup_role(&state.db, group_id, target_user_id)
        .await?
        .ok_or_else(|| {
            Error::NotFound(format!(
                "user {target_user_id} is not a member of group {group_id}"
            ))
        })?;
    match (current, role) {
        (GroupRole::Owner, GroupRole::Owner) | (GroupRole::Member, GroupRole::Member) => {}
        (GroupRole::Member, GroupRole::Owner) => {
            let promoted =
                groups::try_promote_member_to_owner(&state.db, group_id, target_user_id).await?;
            if !promoted {
                return Err(Error::NotFound(format!(
                    "user {target_user_id} is not a member of group {group_id}"
                )));
            }
        }
        (GroupRole::Owner, GroupRole::Member) => {
            if target_user_id != user.user_id {
                return Err(Error::Forbidden(
                    "owners cannot demote other owners; ask them to demote themselves or leave"
                        .to_owned(),
                ));
            }
            let demoted = groups::try_self_demote_owner(&state.db, group_id, user.user_id).await?;
            if !demoted {
                return Err(Error::BadRequest(
                    "cannot demote the last owner; promote another member first or delete the group"
                        .to_owned(),
                ));
            }
        }
    }
    Ok((ReqwestStatusCode::NO_CONTENT, "").into_response())
}

/// `DELETE /api/groups/{id}/members/{user_id}` — remove a non-owner
/// member from the group. Owners only. Owners can never be removed by
/// another owner; an owner who wants to step down must demote themselves
/// or call `/leave`.
///
/// # Errors
///
/// Returns [`Error::Forbidden`] if the caller is not an owner or the
/// target is an owner, or [`Error::NotFound`] if the target is not a
/// member.
pub async fn remove_member(
    user: CurrentUser,
    State(state): State<AppState>,
    Path((group_id, target_user_id)): Path<(Uuid, Uuid)>,
) -> Result<Response, Error> {
    require_owner(&state, group_id, user.user_id).await?;
    let removed = groups::try_remove_non_owner(&state.db, group_id, target_user_id).await?;
    if removed {
        return Ok((ReqwestStatusCode::NO_CONTENT, "").into_response());
    }
    match groups::lookup_role(&state.db, group_id, target_user_id).await? {
        None => Err(Error::NotFound(format!(
            "user {target_user_id} is not a member of group {group_id}"
        ))),
        Some(_) => Err(Error::Forbidden(
            "owners cannot be removed by other owners; the owner must demote themselves or leave"
                .to_owned(),
        )),
    }
}

/// `POST /api/groups/{id}/leave` — the calling user leaves the group. The
/// last owner cannot leave; they must delete the group instead.
///
/// # Errors
///
/// Returns [`Error::BadRequest`] if the caller is the sole owner, or
/// [`Error::NotFound`] if the caller is not a member.
pub async fn leave(
    user: CurrentUser,
    State(state): State<AppState>,
    Path(group_id): Path<Uuid>,
) -> Result<Response, Error> {
    let left = groups::try_leave(&state.db, group_id, user.user_id).await?;
    if left {
        return Ok((ReqwestStatusCode::NO_CONTENT, "").into_response());
    }
    match groups::lookup_role(&state.db, group_id, user.user_id).await? {
        None => Err(Error::NotFound(format!(
            "you are not a member of group {group_id}"
        ))),
        Some(_) => Err(Error::BadRequest(
            "you are the last owner of this group; promote another member to owner or \
             delete the group instead of leaving"
                .to_owned(),
        )),
    }
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
