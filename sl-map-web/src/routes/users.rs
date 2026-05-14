//! User-profile endpoints. The profile is a public-within-the-app view of
//! a user (`user_id`, `username`, `legacy_name`, `created_at`) that any
//! authenticated caller can read; the self-only `DELETE /api/users/me`
//! lets a user remove their own account.
//!
//! Audit columns elsewhere (`groups.created_by`, `saved_notecards.uploaded_by`,
//! `saved_renders.created_by`) are `ON DELETE SET NULL` after migration
//! 0008, so deleting a user anonymises their historical contributions
//! instead of cascading data destruction. The handler still refuses if
//! the caller is the sole owner of any group, because that would leave
//! the group with zero owners — a state the application's invariants
//! do not allow.

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode as ReqwestStatusCode;
use axum::response::{IntoResponse as _, Response};
use axum_extra::extract::cookie::SignedCookieJar;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::auth::{self, CurrentUser, removal_cookie};
use crate::error::Error;
use crate::state::AppState;

/// True if `s` is canonical `#rrggbb` — exactly one leading `#` and six
/// ASCII hex digits. Used to validate the route-colour preference on
/// the way in.
fn is_canonical_hex_color(s: &str) -> bool {
    let mut chars = s.chars();
    if chars.next() != Some('#') {
        return false;
    }
    let mut count = 0_usize;
    for c in chars {
        if !c.is_ascii_hexdigit() {
            return false;
        }
        count = count.saturating_add(1);
    }
    count == 6
}

/// Public view of a registered user, suitable for the profile page and
/// for the creator/uploader link-throughs in the library and groups
/// pages.
#[derive(Debug, Clone, Serialize)]
pub struct UserProfile {
    /// the SL avatar UUID.
    pub user_id: Uuid,
    /// `firstname.lastname` (the login username).
    pub username: String,
    /// `Firstname Lastname` (the legacy display name).
    pub legacy_name: String,
    /// when the account was first registered.
    pub created_at: DateTime<Utc>,
}

/// `GET /api/users/{user_id}` — return the public profile of a user.
///
/// Any authenticated caller may read any registered user's profile —
/// the columns returned (username + legacy name) are already exposed via
/// every notecard / render / group-membership row the user has touched,
/// so making them queryable by UUID is not an additional leak. The
/// alternative — only letting users you share a group with see your
/// profile — would force separate code paths in the library / groups
/// listings just to render the same names already visible there.
///
/// # Errors
///
/// Returns [`Error::NotFound`] if the user does not exist.
pub async fn get(
    _user: CurrentUser,
    State(state): State<AppState>,
    Path(user_id): Path<Uuid>,
) -> Result<Json<UserProfile>, Error> {
    let row: Option<(String, String, DateTime<Utc>)> =
        sqlx::query_as("SELECT username, legacy_name, created_at FROM users WHERE user_id = ?1")
            .bind(user_id.as_bytes().to_vec())
            .fetch_optional(&state.db)
            .await
            .map_err(|err| {
                tracing::error!("user profile lookup failed: {err}");
                Error::Database
            })?;
    let (username, legacy_name, created_at) =
        row.ok_or_else(|| Error::NotFound(format!("user {user_id}")))?;
    Ok(Json(UserProfile {
        user_id,
        username,
        legacy_name,
        created_at,
    }))
}

/// `DELETE /api/users/me` — delete the calling user's account.
///
/// Refuses with [`Error::BadRequest`] if the caller is the sole owner
/// of any group, listing the offending groups so the user knows what
/// to fix. Otherwise the user row is removed in a single statement;
/// FK cascades clean up sessions, memberships, invitations, rate-bucket
/// state, and personal-scope notecards / renders. Group-scope uploads
/// the user contributed survive: the audit FKs are `ON DELETE SET NULL`
/// so the rows stay but no longer name the deleted user.
///
/// The session cookie is cleared in the response so the browser does
/// not keep sending a now-invalid id (the session row itself is gone
/// via cascade, but the cookie clear avoids one extra round-trip
/// before the user is bounced back to /login).
///
/// # Errors
///
/// Returns [`Error::BadRequest`] for sole-ownership; [`Error::Database`]
/// on any underlying query failure.
pub async fn delete_me(
    user: CurrentUser,
    State(state): State<AppState>,
    jar: SignedCookieJar,
) -> Result<(SignedCookieJar, Response), Error> {
    // Sole-owner check: every group where the caller is an `owner` and
    // is the only `owner` blocks deletion. A multi-owner group is fine
    // because the user's own membership cascades and the remaining
    // owner(s) keep the group whole.
    let sole_owner_groups: Vec<(Vec<u8>, String)> = sqlx::query_as(
        "SELECT g.group_id, g.name \
         FROM \"groups\" AS g \
         JOIN group_memberships AS gm \
           ON gm.group_id = g.group_id \
         WHERE gm.user_id = ?1 \
           AND gm.role = 'owner' \
           AND (SELECT COUNT(*) FROM group_memberships \
                WHERE group_id = g.group_id AND role = 'owner') = 1",
    )
    .bind(user.user_id.as_bytes().to_vec())
    .fetch_all(&state.db)
    .await
    .map_err(|err| {
        tracing::error!("sole-owner precheck failed: {err}");
        Error::Database
    })?;
    if !sole_owner_groups.is_empty() {
        let names: Vec<String> = sole_owner_groups.into_iter().map(|(_, n)| n).collect();
        return Err(Error::BadRequest(format!(
            "you are the sole owner of the following group(s): {}. \
             Promote another member to owner or delete the group(s) first.",
            names.join(", ")
        )));
    }

    let result = sqlx::query("DELETE FROM users WHERE user_id = ?1")
        .bind(user.user_id.as_bytes().to_vec())
        .execute(&state.db)
        .await
        .map_err(|err| {
            tracing::error!("delete user failed: {err}");
            Error::Database
        })?;
    if result.rows_affected() == 0 {
        // The row vanished out from under us between the precheck and
        // here — treat as already-deleted from the caller's POV.
        tracing::warn!("delete_me: user row was already gone");
    }
    // Best-effort session-id removal (the cascade already nuked the row,
    // but `delete_session` is a no-op on a missing id, so it is cheap
    // to call defensively).
    if let Some(session_id) = auth::session_id_from_jar(&jar, &state.config.cookie_name) {
        drop(auth::delete_session(&state.db, &session_id).await);
    }
    let cleared = jar.add(removal_cookie(&state.config));
    Ok((cleared, (ReqwestStatusCode::NO_CONTENT, "").into_response()))
}

/// JSON body for `PATCH /api/users/me/preferences`. A `null`
/// `route_color` clears the preference (back to the picker default).
#[derive(Debug, Deserialize)]
pub struct UpdatePreferences {
    /// new route colour as canonical `#rrggbb`, or `None` to clear.
    pub route_color: Option<String>,
}

/// `PATCH /api/users/me/preferences` — update the calling user's
/// preferences. Currently the only preference is `route_color`, the
/// default for the renderer page's route-colour picker.
///
/// # Errors
///
/// Returns [`Error::BadRequest`] if `route_color` is supplied but does
/// not match the canonical `#rrggbb` format; [`Error::Database`] on
/// underlying query failure.
pub async fn update_preferences(
    user: CurrentUser,
    State(state): State<AppState>,
    Json(req): Json<UpdatePreferences>,
) -> Result<Response, Error> {
    if let Some(ref c) = req.route_color
        && !is_canonical_hex_color(c)
    {
        return Err(Error::BadRequest(format!(
            "route_color must be canonical `#rrggbb`, got {c:?}"
        )));
    }
    sqlx::query("UPDATE users SET route_color = ?1 WHERE user_id = ?2")
        .bind(&req.route_color)
        .bind(user.user_id.as_bytes().to_vec())
        .execute(&state.db)
        .await
        .map_err(|err| {
            tracing::error!("update preferences failed: {err}");
            Error::Database
        })?;
    Ok((ReqwestStatusCode::NO_CONTENT, "").into_response())
}
