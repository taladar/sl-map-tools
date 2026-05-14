//! JSON handlers for the authentication endpoints.

use axum::Json;
use axum::extract::State;
use axum::response::{IntoResponse as _, Response};
use axum_extra::extract::cookie::SignedCookieJar;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::auth::{
    self, CurrentUser, LslBearer, SetPasswordTokenRow, generate_token, hash_password, hash_token,
    removal_cookie, verify_password,
};
use crate::client_ip::ClientIp;
use crate::error::Error;
use crate::state::AppState;

/// Minimum length of a new password, in bytes. Long-and-simple beats
/// short-and-complex; we don't enforce complexity rules (per NIST 800-63B
/// guidance) but we want enough characters that brute force is not
/// trivial.
const MIN_PASSWORD_LENGTH: usize = 12;

/// Maximum length of a new password, in bytes. Bounds the Argon2 input so
/// an attacker can't burn CPU per attempt by submitting megabyte-scale
/// "passwords". 128 bytes is far longer than any human-typed passphrase.
const MAX_PASSWORD_LENGTH: usize = 128;

/// Body sent by the LSL script to register a new avatar or refresh the
/// names of an existing one.
#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    /// the avatar's UUID (the SL "agent key").
    pub agent_key: Uuid,
    /// the legacy "Firstname Lastname" form.
    pub legacy_name: String,
    /// the modern `firstname.lastname` username.
    pub username: String,
}

/// Response returned to the LSL script — carries the one-time link the
/// script chats back to the user in-world.
#[derive(Debug, Serialize)]
pub struct RegisterResponse {
    /// full URL the user should open to set their password.
    pub set_password_url: String,
    /// when the token in the URL stops being valid.
    pub expires_at: DateTime<Utc>,
    /// the persisted user record.
    pub user: UserView,
}

/// Public view of a user row, used in API responses.
#[derive(Debug, Serialize)]
pub struct UserView {
    /// the SL avatar UUID.
    pub user_id: Uuid,
    /// `firstname.lastname` username.
    pub username: String,
    /// "Firstname Lastname" legacy display name.
    pub legacy_name: String,
}

/// `POST /api/auth/register` — LSL-facing registration / password-reset
/// trigger. Bearer-authenticated.
///
/// # Errors
///
/// Returns [`Error`] on auth failure, validation failure or DB error.
pub async fn register(
    State(state): State<AppState>,
    _bearer: LslBearer,
    Json(req): Json<RegisterRequest>,
) -> Result<Json<RegisterResponse>, Error> {
    let legacy_name = req.legacy_name.trim();
    let username = req.username.trim();
    if legacy_name.is_empty() || username.is_empty() {
        return Err(Error::BadRequest(
            "legacy_name and username must be non-empty".to_owned(),
        ));
    }
    let now = Utc::now();
    let user_uuid = req.agent_key;
    let user_id_bytes = user_uuid.as_bytes().to_vec();

    // Generate a fresh single-use token, store only the hash.
    let (raw_token, token_hash) = generate_token();
    let expires_at = now
        .checked_add_signed(chrono::Duration::seconds(
            state.config.set_password_token_ttl_seconds,
        ))
        .unwrap_or(chrono::DateTime::<Utc>::MAX_UTC);

    let mut tx = state.db.begin().await.map_err(|err| {
        tracing::error!("register begin tx failed: {err}");
        Error::Database
    })?;

    // UPSERT: on a fresh UUID create the row; on a re-register update the
    // name fields (the user may have paid SL to change their username or
    // legacy display).
    sqlx::query(
        "INSERT INTO users (user_id, legacy_name, username, created_at, updated_at) \
            VALUES (?1, ?2, ?3, ?4, ?4) \
         ON CONFLICT(user_id) DO UPDATE SET \
            legacy_name = excluded.legacy_name, \
            username = excluded.username, \
            updated_at = excluded.updated_at",
    )
    .bind(&user_id_bytes)
    .bind(legacy_name)
    .bind(username)
    .bind(now)
    .execute(&mut *tx)
    .await
    .map_err(|err| {
        tracing::error!("register UPSERT failed: {err}");
        Error::Database
    })?;

    // Invalidate any still-live unused tokens this user already has, so the
    // table only ever carries one outstanding token per user. The periodic
    // sweeper in `auth::run_cleanup` reaps expired-or-used rows on its own
    // schedule; this DELETE bounds the in-between window where repeated
    // re-registers would otherwise accumulate live tokens.
    sqlx::query("DELETE FROM set_password_tokens WHERE user_id = ?1 AND used_at IS NULL")
        .bind(&user_id_bytes)
        .execute(&mut *tx)
        .await
        .map_err(|err| {
            tracing::error!("prior token prune failed: {err}");
            Error::Database
        })?;

    sqlx::query(
        "INSERT INTO set_password_tokens (token_hash, user_id, expires_at, created_at) \
            VALUES (?1, ?2, ?3, ?4)",
    )
    .bind(&token_hash)
    .bind(&user_id_bytes)
    .bind(expires_at)
    .bind(now)
    .execute(&mut *tx)
    .await
    .map_err(|err| {
        tracing::error!("set-password token insert failed: {err}");
        Error::Database
    })?;

    tx.commit().await.map_err(|err| {
        tracing::error!("register commit failed: {err}");
        Error::Database
    })?;

    let base = state.config.public_base_url.trim_end_matches('/');
    let set_password_url = format!("{base}/set-password?token={raw_token}");
    Ok(Json(RegisterResponse {
        set_password_url,
        expires_at,
        user: UserView {
            user_id: user_uuid,
            username: username.to_owned(),
            legacy_name: legacy_name.to_owned(),
        },
    }))
}

/// Body for `POST /api/auth/set-password`.
#[derive(Debug, Deserialize)]
pub struct SetPasswordRequest {
    /// the raw token from the URL query string.
    pub token: String,
    /// the chosen password (plain text over HTTPS; hashed before storage).
    pub new_password: String,
}

/// `POST /api/auth/set-password` — consume a one-time token and store the
/// password hash.
///
/// On success all existing sessions for the user are deleted so a reset
/// invalidates other browsers immediately.
///
/// # Errors
///
/// Returns [`Error::InvalidOrExpiredToken`] for any token-validity failure
/// (missing, wrong length, unknown hash, already used, expired). Other
/// errors come from the DB or hash subsystem.
pub async fn set_password(
    State(state): State<AppState>,
    Json(req): Json<SetPasswordRequest>,
) -> Result<Response, Error> {
    if req.new_password.len() < MIN_PASSWORD_LENGTH {
        return Err(Error::BadRequest(format!(
            "password must be at least {MIN_PASSWORD_LENGTH} characters"
        )));
    }
    if req.new_password.len() > MAX_PASSWORD_LENGTH {
        return Err(Error::BadRequest(format!(
            "password must be at most {MAX_PASSWORD_LENGTH} characters"
        )));
    }
    let Some(token_hash) = hash_token(&req.token) else {
        return Err(Error::InvalidOrExpiredToken);
    };

    let row: Option<SetPasswordTokenRow> = sqlx::query_as(
        "SELECT user_id, expires_at, used_at \
         FROM set_password_tokens \
         WHERE token_hash = ?1",
    )
    .bind(&token_hash)
    .fetch_optional(&state.db)
    .await
    .map_err(|err| {
        tracing::error!("token lookup failed: {err}");
        Error::Database
    })?;

    let Some((user_id_bytes, expires_at, used_at)) = row else {
        return Err(Error::InvalidOrExpiredToken);
    };
    let now = Utc::now();
    if used_at.is_some() || expires_at <= now {
        return Err(Error::InvalidOrExpiredToken);
    }

    let password_hash = hash_password(&req.new_password)?;

    let mut tx = state.db.begin().await.map_err(|err| {
        tracing::error!("failed to begin tx: {err}");
        Error::Database
    })?;
    sqlx::query("UPDATE users SET password_hash = ?1, updated_at = ?2 WHERE user_id = ?3")
        .bind(&password_hash)
        .bind(now)
        .bind(&user_id_bytes)
        .execute(&mut *tx)
        .await
        .map_err(|err| {
            tracing::error!("password update failed: {err}");
            Error::Database
        })?;
    sqlx::query("UPDATE set_password_tokens SET used_at = ?1 WHERE token_hash = ?2")
        .bind(now)
        .bind(&token_hash)
        .execute(&mut *tx)
        .await
        .map_err(|err| {
            tracing::error!("token consume failed: {err}");
            Error::Database
        })?;
    // also invalidate any *other* unused tokens for this user so a stolen
    // link cannot be used after the legitimate one has been clicked.
    sqlx::query(
        "DELETE FROM set_password_tokens WHERE user_id = ?1 AND token_hash != ?2 AND used_at IS NULL",
    )
    .bind(&user_id_bytes)
    .bind(&token_hash)
    .execute(&mut *tx)
    .await
    .map_err(|err| {
        tracing::error!("token cleanup failed: {err}");
        Error::Database
    })?;
    // wipe existing sessions on password change.
    sqlx::query("DELETE FROM sessions WHERE user_id = ?1")
        .bind(&user_id_bytes)
        .execute(&mut *tx)
        .await
        .map_err(|err| {
            tracing::error!("session wipe on password change failed: {err}");
            Error::Database
        })?;
    tx.commit().await.map_err(|err| {
        tracing::error!("failed to commit tx: {err}");
        Error::Database
    })?;
    Ok((axum::http::StatusCode::OK, "password set").into_response())
}

/// Body for `POST /api/auth/login`.
#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    /// Avatar UUID, `firstname.lastname` username, or legacy "Firstname
    /// Lastname". Resolved to a single user row.
    pub identifier: String,
    /// the user's password.
    pub password: String,
}

/// Response from a successful login.
#[derive(Debug, Serialize)]
pub struct LoginResponse {
    /// the now-logged-in user.
    pub user: UserView,
}

/// `POST /api/auth/login` — verify credentials, issue a session cookie.
///
/// # Errors
///
/// Returns [`Error::InvalidCredentials`] if the user is unknown, the
/// password is wrong, or the user has not yet completed the set-password
/// flow.
pub async fn login(
    State(state): State<AppState>,
    jar: SignedCookieJar,
    ClientIp(client_ip): ClientIp,
    Json(req): Json<LoginRequest>,
) -> Result<(SignedCookieJar, Json<LoginResponse>), Error> {
    let identifier = req.identifier.trim();
    if identifier.is_empty() {
        return Err(Error::InvalidCredentials);
    }

    let row = auth::lookup_user_by_identifier(&state.db, identifier).await?;
    let Some((user_id_bytes, legacy_name, username, password_hash)) = row else {
        return Err(Error::InvalidCredentials);
    };
    let Some(stored_hash) = password_hash else {
        return Err(Error::InvalidCredentials);
    };
    if !verify_password(&req.password, &stored_hash)? {
        return Err(Error::InvalidCredentials);
    }
    let user_id = auth::uuid_from_bytes(&user_id_bytes).ok_or(Error::InvalidCredentials)?;
    // Retire any session row whose cookie the client brought to this
    // request before we mint a new one. Defends against a pre-login
    // leaked id surviving the re-login on the same browser — see L17.
    // Only the cookie-carried session is touched; other browsers /
    // devices keep their independent sessions.
    if let Some(session_id) = auth::session_id_from_jar(&jar, &state.config.cookie_name) {
        auth::delete_session(&state.db, &session_id).await?;
    }
    let cookie = auth::create_session(
        &state.db,
        &state.config,
        user_id,
        Some(client_ip.to_string()),
    )
    .await?;
    Ok((
        jar.add(cookie),
        Json(LoginResponse {
            user: UserView {
                user_id,
                username,
                legacy_name,
            },
        }),
    ))
}

/// `POST /api/auth/logout` — delete the server-side session row and clear
/// the cookie on the client.
///
/// # Errors
///
/// Returns [`Error::Database`] on delete failure.
pub async fn logout(
    State(state): State<AppState>,
    jar: SignedCookieJar,
) -> Result<(SignedCookieJar, Response), Error> {
    if let Some(session_id) = auth::session_id_from_jar(&jar, &state.config.cookie_name) {
        auth::delete_session(&state.db, &session_id).await?;
    }
    let cleared = jar.add(removal_cookie(&state.config));
    Ok((
        cleared,
        (axum::http::StatusCode::OK, "logged out").into_response(),
    ))
}

/// `GET /api/auth/me` — return the currently authenticated user, used by
/// the front-end to populate the header bar.
pub async fn me(user: CurrentUser) -> Json<UserView> {
    Json(UserView {
        user_id: user.user_id,
        username: user.username,
        legacy_name: user.legacy_name,
    })
}
