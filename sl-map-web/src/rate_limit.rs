//! Token-bucket rate limiter for authenticated-user create endpoints,
//! persisted in the `rate_buckets` table.
//!
//! Each `(category, user)` pair has its own bucket. A bucket has a
//! per-category capacity and a per-category refill interval (one token
//! per `seconds_per_token` seconds). On each acquisition the bucket is
//! refilled based on elapsed wall-clock time (clamped to capacity) and
//! one token is deducted; if fewer than one token is available the
//! acquisition returns [`Error::TooManyRequests`] with a conservative
//! `Retry-After` value equal to one refill interval.
//!
//! The refill and deduction happen in a single SQLite UPSERT so two
//! concurrent acquisitions for the same key cannot both pass — SQLite
//! serialises the write and the `ON CONFLICT DO UPDATE … WHERE`
//! clause re-checks the post-refill token count on the second arrival.

use chrono::Utc;
use sqlx::SqlitePool;
use uuid::Uuid;

use crate::error::Error;

/// Categories of rate-limited create endpoints. The string form maps
/// directly to the `rate_buckets.category` column; the CHECK constraint
/// in migration `0004_rate_buckets.sql` lists the same three values.
#[derive(Debug, Clone, Copy)]
pub enum RateCategory {
    /// `POST /api/groups` — creating a new group.
    GroupCreate,
    /// `POST /api/notecards` — saving a notecard (personal or group).
    NotecardCreate,
    /// `POST /api/groups/{id}/invitations` — sending an invitation.
    InvitationCreate,
}

impl RateCategory {
    /// Persisted form used in the `category` column.
    const fn as_str(self) -> &'static str {
        match self {
            Self::GroupCreate => "group_create",
            Self::NotecardCreate => "notecard_create",
            Self::InvitationCreate => "invitation_create",
        }
    }

    /// `(capacity, seconds_per_token)` for this category. The refill
    /// rate per hour is `3600 / seconds_per_token`.
    ///
    /// - `GroupCreate`: 12 burst, 1 token / 20 min (3 / hour)
    /// - `NotecardCreate`: 60 burst, 1 token / 6 min (10 / hour)
    /// - `InvitationCreate`: 250 burst, 1 token / 6 min (10 / hour)
    const fn parameters(self) -> (u32, u32) {
        match self {
            Self::GroupCreate => (12, 1200),
            Self::NotecardCreate => (60, 360),
            Self::InvitationCreate => (250, 360),
        }
    }
}

/// Try to acquire one token from `category`'s bucket for `user_id`.
/// Returns `Ok(())` on success and [`Error::TooManyRequests`] with a
/// `retry_after_secs` equal to the per-category refill interval on
/// failure.
///
/// # Errors
///
/// - [`Error::TooManyRequests`] when the bucket has fewer than 1 token
///   after refill.
/// - [`Error::Database`] if the UPSERT fails for any reason other than
///   the conditional-update no-op (the lookup query is logged via
///   `tracing::error!` before being collapsed).
pub async fn try_acquire(
    db: &SqlitePool,
    category: RateCategory,
    user_id: Uuid,
) -> Result<(), Error> {
    let (capacity, seconds_per_token) = category.parameters();
    let now = Utc::now();
    // A single atomic UPSERT. On first request for this (category,
    // user) the INSERT path runs and the new row is created with
    // `capacity - 1` tokens. On subsequent requests the ON CONFLICT
    // path refills based on the elapsed time since `last_refill_at`,
    // clamps at capacity, and deducts 1 — but only if the post-refill
    // count is at least 1, otherwise the UPDATE is filtered out by the
    // WHERE clause and `RETURNING` yields no row.
    let row: Option<(f64,)> = sqlx::query_as(
        "INSERT INTO rate_buckets (category, user_id, tokens, last_refill_at) \
         VALUES (?1, ?2, ?3, ?4) \
         ON CONFLICT(category, user_id) DO UPDATE SET \
           tokens = MIN(?5, rate_buckets.tokens + \
                       (julianday(excluded.last_refill_at) \
                        - julianday(rate_buckets.last_refill_at)) * 86400.0 / ?6) - 1, \
           last_refill_at = excluded.last_refill_at \
         WHERE MIN(?5, rate_buckets.tokens + \
                       (julianday(excluded.last_refill_at) \
                        - julianday(rate_buckets.last_refill_at)) * 86400.0 / ?6) >= 1 \
         RETURNING tokens",
    )
    .bind(category.as_str())
    .bind(user_id.as_bytes().to_vec())
    .bind(f64::from(capacity) - 1.0)
    .bind(now)
    .bind(f64::from(capacity))
    .bind(f64::from(seconds_per_token))
    .fetch_optional(db)
    .await
    .map_err(|err| {
        tracing::error!("rate_limit acquire failed: {err}");
        Error::Database
    })?;
    if row.is_some() {
        Ok(())
    } else {
        Err(Error::TooManyRequests {
            retry_after_secs: u64::from(seconds_per_token),
        })
    }
}
