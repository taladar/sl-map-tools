//! Authentication primitives: extractors, middleware, password hashing,
//! token generation, and session helpers.

use std::time::Duration;

use argon2::{Argon2, PasswordHash, PasswordHasher as _, PasswordVerifier as _};
use axum::extract::{FromRequestParts, Request, State};
use axum::http::header;
use axum::http::request::Parts;
use axum::middleware::Next;
use axum::response::{IntoResponse as _, Redirect, Response};
use axum_extra::extract::cookie::{Cookie, Key, SameSite, SignedCookieJar};
use base64::Engine as _;
use chrono::{DateTime, Utc};
use password_hash::SaltString;
use rand::RngCore as _;
use rand::rngs::OsRng;
use sha2::{Digest as _, Sha256};
use sqlx::SqlitePool;
use subtle::ConstantTimeEq as _;
use uuid::Uuid;

use crate::config::Config;
use crate::error::Error;
use crate::state::AppState;

/// `(user_id_bytes, legacy_name, username, sessions.expires_at)` — the
/// columns selected in the [`CurrentUser`] extractor's session/user join.
pub type SessionRow = (Vec<u8>, String, String, DateTime<Utc>);

/// `(user_id_bytes, expires_at, used_at)` — columns selected when looking
/// up a set-password token.
pub type SetPasswordTokenRow = (Vec<u8>, DateTime<Utc>, Option<DateTime<Utc>>);

/// Cookie attributes used for the session cookie. Centralised so the login
/// handler and the logout (cookie-removal) handler agree on `Path`, `Secure`,
/// `SameSite`, and the cookie name.
fn build_session_cookie<'a>(
    name: String,
    value: String,
    cfg: &Config,
    max_age_seconds: i64,
) -> Cookie<'a> {
    let mut cookie = Cookie::new(name, value);
    cookie.set_path("/");
    cookie.set_http_only(true);
    cookie.set_secure(cfg.cookie_secure);
    cookie.set_same_site(SameSite::Lax);
    cookie.set_max_age(time::Duration::seconds(max_age_seconds));
    cookie
}

/// Build a removal cookie that clears the session on the client.
#[must_use]
pub fn removal_cookie<'a>(cfg: &Config) -> Cookie<'a> {
    let mut cookie = Cookie::new(cfg.cookie_name.clone(), String::new());
    cookie.set_path("/");
    cookie.set_http_only(true);
    cookie.set_secure(cfg.cookie_secure);
    cookie.set_same_site(SameSite::Lax);
    cookie.set_max_age(time::Duration::seconds(0));
    cookie
}

/// Information about the currently authenticated user, attached to handlers
/// that opt in to the [`CurrentUser`] extractor.
#[derive(Debug, Clone)]
pub struct CurrentUser {
    /// the SL avatar UUID (primary key in the `users` table).
    pub user_id: Uuid,
    /// the legacy "Firstname Lastname" form, used for UI display.
    pub legacy_name: String,
    /// the `firstname.lastname` form, used as the login username.
    pub username: String,
}

impl FromRequestParts<AppState> for CurrentUser {
    type Rejection = Error;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let jar = SignedCookieJar::<Key>::from_headers(&parts.headers, state.cookie_key.clone());
        let Some(cookie) = jar.get(&state.config.cookie_name) else {
            return Err(Error::Unauthenticated);
        };
        let Some(session_id) = decode_session_id(cookie.value()) else {
            return Err(Error::Unauthenticated);
        };
        let session_id_hash = Sha256::digest(session_id).to_vec();
        let now = Utc::now();
        // load the session and its user in one shot; reject if expired.
        let row: Option<SessionRow> = sqlx::query_as(
            "SELECT users.user_id, users.legacy_name, users.username, sessions.expires_at \
             FROM sessions \
             JOIN users ON users.user_id = sessions.user_id \
             WHERE sessions.session_id_hash = ?1",
        )
        .bind(session_id_hash.as_slice())
        .fetch_optional(&state.db)
        .await
        .map_err(|err| {
            tracing::error!("session lookup failed: {err}");
            Error::Database
        })?;
        let Some((uid_bytes, legacy_name, username, expires_at)) = row else {
            return Err(Error::Unauthenticated);
        };
        if expires_at <= now {
            return Err(Error::Unauthenticated);
        }
        let user_id = uuid_from_bytes(&uid_bytes).ok_or(Error::Unauthenticated)?;
        // best-effort bump of last_seen_at; failures here should not break
        // request handling.
        if let Err(err) =
            sqlx::query("UPDATE sessions SET last_seen_at = ?1 WHERE session_id_hash = ?2")
                .bind(now)
                .bind(session_id_hash.as_slice())
                .execute(&state.db)
                .await
        {
            tracing::warn!("failed to bump sessions.last_seen_at: {err}");
        }
        Ok(Self {
            user_id,
            legacy_name,
            username,
        })
    }
}

/// Middleware that enforces a valid session on the wrapped routes.
///
/// On reject:
/// * a GET request that prefers `text/html` is redirected to `/login?next=...`
/// * any other request gets a JSON 401.
///
/// # Errors
///
/// Returns the [`Error::Unauthenticated`] variant; the
/// [`axum::response::IntoResponse`] impl will turn it into the appropriate
/// response.
pub async fn require_session(State(state): State<AppState>, req: Request, next: Next) -> Response {
    let (mut parts, body) = req.into_parts();
    match CurrentUser::from_request_parts(&mut parts, &state).await {
        Ok(user) => {
            parts.extensions.insert(user);
            let req = Request::from_parts(parts, body);
            next.run(req).await
        }
        Err(_) => {
            let wants_html = parts.method == axum::http::Method::GET && prefers_html(&parts);
            if wants_html {
                let next_url = parts
                    .uri
                    .path_and_query()
                    .map_or_else(|| "/".to_owned(), |pq| pq.as_str().to_owned());
                let encoded = percent_encode(&next_url).unwrap_or_else(|| "/".to_owned());
                let target = format!("/login?next={encoded}");
                Redirect::to(&target).into_response()
            } else {
                Error::Unauthenticated.into_response()
            }
        }
    }
}

/// Heuristic: does the request prefer `text/html`? We don't need a full
/// content-negotiation parser; presence of `text/html` in `Accept` is enough
/// for the redirect-vs-401 decision.
fn prefers_html(parts: &Parts) -> bool {
    parts
        .headers
        .get(header::ACCEPT)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|s| s.contains("text/html"))
}

/// Percent-encode a path so it's safe inside a query parameter. Returns
/// `None` only if the input contains characters that cannot be represented.
fn percent_encode(input: &str) -> Option<String> {
    // Restrict to characters that are safe unescaped inside a query
    // parameter value. Everything else is percent-encoded.
    let mut out = String::with_capacity(input.len());
    for byte in input.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~' | b'/') {
            out.push(char::from(byte));
        } else {
            use std::fmt::Write as _;
            write!(out, "%{byte:02X}").ok()?;
        }
    }
    Some(out)
}

/// Bearer-token guard for the LSL-facing registration endpoint. Constant-
/// time-compares against the configured shared secret.
#[derive(Debug)]
pub struct LslBearer;

impl FromRequestParts<AppState> for LslBearer {
    type Rejection = Error;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let Some(value) = parts.headers.get(header::AUTHORIZATION) else {
            return Err(Error::Unauthenticated);
        };
        let bytes = value.as_bytes();
        let prefix = b"Bearer ";
        if bytes.len() <= prefix.len() || !bytes.starts_with(prefix) {
            return Err(Error::Unauthenticated);
        }
        let presented = bytes.get(prefix.len()..).unwrap_or(&[]);
        let expected = state.config.lsl_registration_bearer_token.as_bytes();
        // Run ct_eq over `expected.len()` bytes on every path so the
        // mismatched-length case takes the same time as a wrong-content
        // compare and the bearer's length doesn't leak via timing.
        let len_ok = presented.len() == expected.len();
        let lhs = if len_ok { presented } else { expected };
        let eq: bool = lhs.ct_eq(expected).into();
        if len_ok && eq {
            Ok(Self)
        } else {
            Err(Error::Unauthenticated)
        }
    }
}

/// Hash a password using Argon2id with default parameters and a fresh
/// random salt.
///
/// # Errors
///
/// Returns [`Error::PasswordHash`] if the underlying hasher fails (which
/// should not happen in practice — it can only fail on out-of-memory).
pub fn hash_password(password: &str) -> Result<String, Error> {
    let salt = SaltString::generate(&mut OsRng);
    let argon = Argon2::default();
    let hash = argon
        .hash_password(password.as_bytes(), &salt)
        .map_err(|err| Error::PasswordHash(err.to_string()))?;
    Ok(hash.to_string())
}

/// Verify a password against a PHC-encoded Argon2 hash. Returns `Ok(false)`
/// on mismatch and `Ok(true)` on match.
///
/// # Errors
///
/// Returns [`Error::PasswordHash`] if the stored hash cannot be parsed.
pub fn verify_password(password: &str, stored_hash: &str) -> Result<bool, Error> {
    let parsed =
        PasswordHash::new(stored_hash).map_err(|err| Error::PasswordHash(err.to_string()))?;
    match Argon2::default().verify_password(password.as_bytes(), &parsed) {
        Ok(()) => Ok(true),
        Err(argon2::password_hash::Error::Password) => Ok(false),
        Err(err) => Err(Error::PasswordHash(err.to_string())),
    }
}

/// Generate a fresh 32-byte random token and return both the raw bytes
/// (to be base64-encoded into the link) and the SHA-256 hash (to be stored
/// in the DB).
#[must_use]
pub fn generate_token() -> (String, Vec<u8>) {
    let mut raw = [0_u8; 32];
    OsRng.fill_bytes(&mut raw);
    let encoded = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(raw);
    let hash = Sha256::digest(raw).to_vec();
    (encoded, hash)
}

/// Hash an incoming token string (the one carried in the URL) so it can be
/// looked up in the `set_password_tokens` table.
#[must_use]
pub fn hash_token(raw_token: &str) -> Option<Vec<u8>> {
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(raw_token)
        .ok()?;
    if bytes.len() != 32 {
        return None;
    }
    Some(Sha256::digest(&bytes).to_vec())
}

/// Generate a fresh 32-byte session id. The id is what goes into the cookie
/// (base64-encoded, then signed) and what we look up in the `sessions`
/// table.
#[must_use]
pub fn generate_session_id() -> ([u8; 32], String) {
    let mut raw = [0_u8; 32];
    OsRng.fill_bytes(&mut raw);
    let encoded = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(raw);
    (raw, encoded)
}

/// Decode a session-id cookie value back into the 32 raw bytes.
fn decode_session_id(value: &str) -> Option<[u8; 32]> {
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(value)
        .ok()?;
    if bytes.len() != 32 {
        return None;
    }
    let mut out = [0_u8; 32];
    out.copy_from_slice(&bytes);
    Some(out)
}

/// Convert a 16-byte SQLite BLOB back into a [`Uuid`]. Returns `None` if the
/// blob is the wrong length.
#[must_use]
pub fn uuid_from_bytes(bytes: &[u8]) -> Option<Uuid> {
    let arr: [u8; 16] = bytes.try_into().ok()?;
    Some(Uuid::from_bytes(arr))
}

/// Insert a fresh session for the given user and produce the signed cookie
/// to set on the response.
///
/// # Errors
///
/// Returns [`Error::Database`] on insert failure.
pub async fn create_session(
    pool: &SqlitePool,
    cfg: &Config,
    user_id: Uuid,
    client_ip: Option<String>,
) -> Result<Cookie<'static>, Error> {
    let (raw, encoded) = generate_session_id();
    let now = Utc::now();
    let expires_at = now
        .checked_add_signed(chrono::Duration::seconds(cfg.session_ttl_seconds))
        .unwrap_or(chrono::DateTime::<Utc>::MAX_UTC);
    let session_id_hash = Sha256::digest(raw).to_vec();
    sqlx::query(
        "INSERT INTO sessions \
            (session_id_hash, user_id, expires_at, created_at, last_seen_at, client_ip) \
         VALUES (?1, ?2, ?3, ?4, ?4, ?5)",
    )
    .bind(session_id_hash)
    .bind(user_id.as_bytes().to_vec())
    .bind(expires_at)
    .bind(now)
    .bind(client_ip)
    .execute(pool)
    .await
    .map_err(|err| {
        tracing::error!("failed to insert session row: {err}");
        Error::Database
    })?;
    Ok(build_session_cookie(
        cfg.cookie_name.clone(),
        encoded,
        cfg,
        cfg.session_ttl_seconds,
    ))
}

/// Delete a session row by the raw 32-byte id from the cookie. Used on
/// logout. Hashes internally so callers continue to pass cookie bytes.
///
/// # Errors
///
/// Returns [`Error::Database`] on delete failure.
pub async fn delete_session(pool: &SqlitePool, session_id: &[u8]) -> Result<(), Error> {
    let session_id_hash = Sha256::digest(session_id).to_vec();
    sqlx::query("DELETE FROM sessions WHERE session_id_hash = ?1")
        .bind(session_id_hash)
        .execute(pool)
        .await
        .map_err(|err| {
            tracing::error!("failed to delete session row: {err}");
            Error::Database
        })?;
    Ok(())
}

/// Look up the raw session id from the cookie jar of the current request.
/// Returns `None` if there is no cookie or the cookie does not decode.
#[must_use]
pub fn session_id_from_jar(jar: &SignedCookieJar, cookie_name: &str) -> Option<[u8; 32]> {
    let cookie = jar.get(cookie_name)?;
    decode_session_id(cookie.value())
}

/// `(user_id_bytes, legacy_name, username, password_hash)` — the four
/// columns returned by every users lookup variant.
pub type UserRow = (Vec<u8>, String, String, Option<String>);

/// Look up a user row by identifier. Tries (in order): UUID parse,
/// `firstname.lastname` username, "Firstname Lastname" legacy name. Used by
/// both the login flow and the invitation creation flow, which is why it
/// lives here in `auth.rs` rather than in a route module.
///
/// # Errors
///
/// Returns [`Error::Database`] if any of the DB lookups fail.
pub async fn lookup_user_by_identifier(
    pool: &SqlitePool,
    identifier: &str,
) -> Result<Option<UserRow>, Error> {
    if let Ok(uuid) = Uuid::parse_str(identifier) {
        let bytes = uuid.as_bytes().to_vec();
        let row: Option<UserRow> = sqlx::query_as(
            "SELECT user_id, legacy_name, username, password_hash FROM users WHERE user_id = ?1",
        )
        .bind(bytes)
        .fetch_optional(pool)
        .await
        .map_err(|err| {
            tracing::error!("user lookup (uuid) failed: {err}");
            Error::Database
        })?;
        if row.is_some() {
            return Ok(row);
        }
    }
    let by_username: Option<UserRow> = sqlx::query_as(
        "SELECT user_id, legacy_name, username, password_hash FROM users WHERE username = ?1",
    )
    .bind(identifier)
    .fetch_optional(pool)
    .await
    .map_err(|err| {
        tracing::error!("user lookup (username) failed: {err}");
        Error::Database
    })?;
    if by_username.is_some() {
        return Ok(by_username);
    }
    let by_legacy: Option<UserRow> = sqlx::query_as(
        "SELECT user_id, legacy_name, username, password_hash FROM users WHERE legacy_name = ?1",
    )
    .bind(identifier)
    .fetch_optional(pool)
    .await
    .map_err(|err| {
        tracing::error!("user lookup (legacy_name) failed: {err}");
        Error::Database
    })?;
    Ok(by_legacy)
}

/// Background task that prunes expired sessions and set-password tokens.
/// Spawned alongside the existing job evictor in `bin/sl_map_web.rs`.
pub async fn run_cleanup(pool: SqlitePool) {
    let mut interval = tokio::time::interval(Duration::from_secs(300));
    loop {
        interval.tick().await;
        let now = Utc::now();
        if let Err(err) = sqlx::query("DELETE FROM sessions WHERE expires_at <= ?1")
            .bind(now)
            .execute(&pool)
            .await
        {
            tracing::warn!("session cleanup failed: {err}");
        }
        if let Err(err) = sqlx::query(
            "DELETE FROM set_password_tokens WHERE expires_at <= ?1 OR used_at IS NOT NULL",
        )
        .bind(now)
        .execute(&pool)
        .await
        {
            tracing::warn!("set-password token cleanup failed: {err}");
        }
    }
}
