//! Database-level helpers for groups, memberships, and invitations.
//!
//! Higher-level permission checks live in [`crate::library`]; this module
//! exposes the typed DB primitives those checks (and the route handlers)
//! build on.

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::SqlitePool;
use uuid::Uuid;

use crate::auth::uuid_from_bytes;
use crate::error::Error;

/// Role a user has within a group.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum GroupRole {
    /// Full read/write access; can invite, remove, promote, demote, and
    /// delete the group itself.
    Owner,
    /// Read-only access. Only sees finished renders in the group library.
    Member,
}

impl GroupRole {
    /// String form stored in the `group_memberships.role` column.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Owner => "owner",
            Self::Member => "member",
        }
    }

    /// Parse a role string from the DB or a request body.
    ///
    /// # Errors
    ///
    /// Returns [`Error::BadRequest`] for any value other than `owner` or
    /// `member`.
    pub fn parse(raw: &str) -> Result<Self, Error> {
        match raw {
            "owner" => Ok(Self::Owner),
            "member" => Ok(Self::Member),
            other => Err(Error::BadRequest(format!("unknown role `{other}`"))),
        }
    }
}

/// Status of a group invitation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum InvitationStatus {
    /// Awaiting the invitee's decision.
    Pending,
    /// The invitee accepted; a membership row was created.
    Accepted,
    /// The invitee rejected the invitation.
    Rejected,
}

impl InvitationStatus {
    /// String form stored in the `group_invitations.status` column.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Accepted => "accepted",
            Self::Rejected => "rejected",
        }
    }
}

/// Public view of a group, returned by listing/get endpoints.
#[derive(Debug, Clone, Serialize)]
pub struct GroupView {
    /// the group's identifier.
    pub group_id: Uuid,
    /// the group's display name.
    pub name: String,
    /// the avatar that created the group (immutable).
    pub created_by: Uuid,
    /// when the group was created.
    pub created_at: DateTime<Utc>,
    /// when the group's metadata was last updated (e.g. renamed).
    pub updated_at: DateTime<Utc>,
    /// the calling user's role in this group.
    pub my_role: GroupRole,
}

/// One member of a group with the user's display fields denormalised in.
#[derive(Debug, Clone, Serialize)]
pub struct GroupMemberView {
    /// the member's UUID.
    pub user_id: Uuid,
    /// `firstname.lastname`.
    pub username: String,
    /// `Firstname Lastname`.
    pub legacy_name: String,
    /// the member's role in the group.
    pub role: GroupRole,
    /// when the membership row was created.
    pub created_at: DateTime<Utc>,
}

/// Look up the role a given user has in a given group, or `None` if they are
/// not a member.
///
/// # Errors
///
/// Returns [`Error::Database`] on lookup failure.
pub async fn lookup_role(
    db: &SqlitePool,
    group_id: Uuid,
    user_id: Uuid,
) -> Result<Option<GroupRole>, Error> {
    let row: Option<(String,)> =
        sqlx::query_as("SELECT role FROM group_memberships WHERE group_id = ?1 AND user_id = ?2")
            .bind(group_id.as_bytes().to_vec())
            .bind(user_id.as_bytes().to_vec())
            .fetch_optional(db)
            .await
            .map_err(|err| {
                tracing::error!("group role lookup failed: {err}");
                Error::Database
            })?;
    row.map(|(role,)| GroupRole::parse(&role)).transpose()
}

/// Count the number of owners a group currently has. Used by the application
/// layer to enforce the "at least one owner" invariant when removing,
/// demoting, or leaving.
///
/// # Errors
///
/// Returns [`Error::Database`] on count failure.
pub async fn count_owners(db: &SqlitePool, group_id: Uuid) -> Result<i64, Error> {
    let (count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM group_memberships WHERE group_id = ?1 AND role = 'owner'",
    )
    .bind(group_id.as_bytes().to_vec())
    .fetch_one(db)
    .await
    .map_err(|err| {
        tracing::error!("owner count failed: {err}");
        Error::Database
    })?;
    Ok(count)
}

/// Verify that the given group exists. Returns [`Error::NotFound`] if not.
///
/// # Errors
///
/// Returns [`Error::Database`] on lookup failure, [`Error::NotFound`] if the
/// group does not exist.
pub async fn require_exists(db: &SqlitePool, group_id: Uuid) -> Result<(), Error> {
    let row: Option<(i64,)> = sqlx::query_as("SELECT 1 FROM groups WHERE group_id = ?1")
        .bind(group_id.as_bytes().to_vec())
        .fetch_optional(db)
        .await
        .map_err(|err| {
            tracing::error!("group existence check failed: {err}");
            Error::Database
        })?;
    if row.is_none() {
        return Err(Error::NotFound(format!("group {group_id}")));
    }
    Ok(())
}

/// Insert a new group plus the creator's owner membership in one transaction.
/// Returns the newly assigned `group_id`.
///
/// # Errors
///
/// Returns [`Error::Database`] on insert failure.
pub async fn create_group(db: &SqlitePool, name: &str, creator: Uuid) -> Result<Uuid, Error> {
    let group_id = Uuid::new_v4();
    let now = Utc::now();
    let mut tx = db.begin().await.map_err(|err| {
        tracing::error!("begin tx for group create failed: {err}");
        Error::Database
    })?;
    sqlx::query(
        "INSERT INTO groups (group_id, name, created_by, created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, ?4)",
    )
    .bind(group_id.as_bytes().to_vec())
    .bind(name)
    .bind(creator.as_bytes().to_vec())
    .bind(now)
    .execute(&mut *tx)
    .await
    .map_err(|err| {
        tracing::error!("insert groups row failed: {err}");
        Error::Database
    })?;
    sqlx::query(
        "INSERT INTO group_memberships (group_id, user_id, role, created_at) \
         VALUES (?1, ?2, 'owner', ?3)",
    )
    .bind(group_id.as_bytes().to_vec())
    .bind(creator.as_bytes().to_vec())
    .bind(now)
    .execute(&mut *tx)
    .await
    .map_err(|err| {
        tracing::error!("insert owner membership failed: {err}");
        Error::Database
    })?;
    tx.commit().await.map_err(|err| {
        tracing::error!("commit group create failed: {err}");
        Error::Database
    })?;
    Ok(group_id)
}

/// Row shape for `list_members`: `(user_id, username, legacy_name, role,
/// created_at)`.
type MemberRow = (Vec<u8>, String, String, String, DateTime<Utc>);

/// List all members of a group, joined with the users table for display
/// fields.
///
/// # Errors
///
/// Returns [`Error::Database`] on query failure.
pub async fn list_members(db: &SqlitePool, group_id: Uuid) -> Result<Vec<GroupMemberView>, Error> {
    let rows: Vec<MemberRow> = sqlx::query_as(
        "SELECT users.user_id, users.username, users.legacy_name, \
                group_memberships.role, group_memberships.created_at \
         FROM group_memberships \
         JOIN users ON users.user_id = group_memberships.user_id \
         WHERE group_memberships.group_id = ?1 \
         ORDER BY group_memberships.role DESC, users.username ASC",
    )
    .bind(group_id.as_bytes().to_vec())
    .fetch_all(db)
    .await
    .map_err(|err| {
        tracing::error!("list members failed: {err}");
        Error::Database
    })?;
    let mut out = Vec::with_capacity(rows.len());
    for (uid_bytes, username, legacy_name, role, created_at) in rows {
        let user_id =
            uuid_from_bytes(&uid_bytes).ok_or_else(|| Error::BadRequest("bad uuid".to_owned()))?;
        out.push(GroupMemberView {
            user_id,
            username,
            legacy_name,
            role: GroupRole::parse(&role)?,
            created_at,
        });
    }
    Ok(out)
}

/// Row shape for `list_for_user`: `(group_id, name, created_by, created_at,
/// updated_at, my_role)`.
type GroupRow = (
    Vec<u8>,
    String,
    Vec<u8>,
    DateTime<Utc>,
    DateTime<Utc>,
    String,
);

/// List the groups the user belongs to, with their role in each.
///
/// # Errors
///
/// Returns [`Error::Database`] on query failure.
pub async fn list_for_user(db: &SqlitePool, user_id: Uuid) -> Result<Vec<GroupView>, Error> {
    let rows: Vec<GroupRow> = sqlx::query_as(
        "SELECT groups.group_id, groups.name, groups.created_by, \
                groups.created_at, groups.updated_at, group_memberships.role \
         FROM groups \
         JOIN group_memberships ON group_memberships.group_id = groups.group_id \
         WHERE group_memberships.user_id = ?1 \
         ORDER BY groups.name ASC",
    )
    .bind(user_id.as_bytes().to_vec())
    .fetch_all(db)
    .await
    .map_err(|err| {
        tracing::error!("list groups for user failed: {err}");
        Error::Database
    })?;
    let mut out = Vec::with_capacity(rows.len());
    for (gid_bytes, name, created_by_bytes, created_at, updated_at, role) in rows {
        let group_id = uuid_from_bytes(&gid_bytes)
            .ok_or_else(|| Error::BadRequest("bad group uuid".to_owned()))?;
        let created_by = uuid_from_bytes(&created_by_bytes)
            .ok_or_else(|| Error::BadRequest("bad creator uuid".to_owned()))?;
        out.push(GroupView {
            group_id,
            name,
            created_by,
            created_at,
            updated_at,
            my_role: GroupRole::parse(&role)?,
        });
    }
    Ok(out)
}

/// Fetch a single group view if the user is a member of it.
///
/// # Errors
///
/// Returns [`Error::Database`] on query failure or [`Error::NotFound`] if the
/// group does not exist or the user is not a member.
pub async fn get_for_user(
    db: &SqlitePool,
    group_id: Uuid,
    user_id: Uuid,
) -> Result<GroupView, Error> {
    type GetGroupRow = (String, Vec<u8>, DateTime<Utc>, DateTime<Utc>, String);
    let row: Option<GetGroupRow> = sqlx::query_as(
        "SELECT groups.name, groups.created_by, groups.created_at, groups.updated_at, \
                group_memberships.role \
         FROM groups \
         JOIN group_memberships ON group_memberships.group_id = groups.group_id \
         WHERE groups.group_id = ?1 AND group_memberships.user_id = ?2",
    )
    .bind(group_id.as_bytes().to_vec())
    .bind(user_id.as_bytes().to_vec())
    .fetch_optional(db)
    .await
    .map_err(|err| {
        tracing::error!("get group for user failed: {err}");
        Error::Database
    })?;
    let (name, created_by_bytes, created_at, updated_at, role) =
        row.ok_or_else(|| Error::NotFound(format!("group {group_id}")))?;
    let created_by = uuid_from_bytes(&created_by_bytes)
        .ok_or_else(|| Error::BadRequest("bad creator uuid".to_owned()))?;
    Ok(GroupView {
        group_id,
        name,
        created_by,
        created_at,
        updated_at,
        my_role: GroupRole::parse(&role)?,
    })
}

/// Rename a group, bumping `updated_at`.
///
/// # Errors
///
/// Returns [`Error::Database`] on update failure.
pub async fn rename_group(db: &SqlitePool, group_id: Uuid, name: &str) -> Result<(), Error> {
    let now = Utc::now();
    sqlx::query("UPDATE groups SET name = ?1, updated_at = ?2 WHERE group_id = ?3")
        .bind(name)
        .bind(now)
        .bind(group_id.as_bytes().to_vec())
        .execute(db)
        .await
        .map_err(|err| {
            tracing::error!("rename group failed: {err}");
            Error::Database
        })?;
    Ok(())
}

/// Delete a group. Cascades remove memberships, pending invitations and any
/// group-owned saved notecards / renders.
///
/// # Errors
///
/// Returns [`Error::Database`] on delete failure.
pub async fn delete_group(db: &SqlitePool, group_id: Uuid) -> Result<(), Error> {
    sqlx::query("DELETE FROM groups WHERE group_id = ?1")
        .bind(group_id.as_bytes().to_vec())
        .execute(db)
        .await
        .map_err(|err| {
            tracing::error!("delete group failed: {err}");
            Error::Database
        })?;
    Ok(())
}

/// Set a member's role to the supplied value.
///
/// # Errors
///
/// Returns [`Error::NotFound`] if the user is not a member,
/// [`Error::Database`] on update failure.
pub async fn set_role(
    db: &SqlitePool,
    group_id: Uuid,
    user_id: Uuid,
    role: GroupRole,
) -> Result<(), Error> {
    let result =
        sqlx::query("UPDATE group_memberships SET role = ?1 WHERE group_id = ?2 AND user_id = ?3")
            .bind(role.as_str())
            .bind(group_id.as_bytes().to_vec())
            .bind(user_id.as_bytes().to_vec())
            .execute(db)
            .await
            .map_err(|err| {
                tracing::error!("update member role failed: {err}");
                Error::Database
            })?;
    if result.rows_affected() == 0 {
        return Err(Error::NotFound(format!(
            "user {user_id} is not a member of group {group_id}"
        )));
    }
    Ok(())
}

/// Remove a member from a group.
///
/// # Errors
///
/// Returns [`Error::NotFound`] if the user is not a member,
/// [`Error::Database`] on delete failure.
pub async fn remove_member(db: &SqlitePool, group_id: Uuid, user_id: Uuid) -> Result<(), Error> {
    let result = sqlx::query("DELETE FROM group_memberships WHERE group_id = ?1 AND user_id = ?2")
        .bind(group_id.as_bytes().to_vec())
        .bind(user_id.as_bytes().to_vec())
        .execute(db)
        .await
        .map_err(|err| {
            tracing::error!("delete membership failed: {err}");
            Error::Database
        })?;
    if result.rows_affected() == 0 {
        return Err(Error::NotFound(format!(
            "user {user_id} is not a member of group {group_id}"
        )));
    }
    Ok(())
}
