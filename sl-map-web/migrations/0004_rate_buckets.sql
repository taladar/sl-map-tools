-- Token-bucket rate-limiter state for authenticated-user create
-- endpoints. One row per (category, user). `tokens` may be fractional
-- (SQLite REAL); on each acquisition the bucket is refilled by
-- `(now - last_refill_at) / seconds_per_token`, clamped to the
-- per-category capacity, and 1 token is deducted. If fewer than 1 token
-- is available after refill the request is rejected with HTTP 429.

CREATE TABLE rate_buckets (
    category TEXT NOT NULL CHECK (
        category IN ('group_create', 'notecard_create', 'invitation_create')
    ),
    user_id BLOB NOT NULL REFERENCES users (user_id) ON DELETE CASCADE,
    tokens REAL NOT NULL,
    last_refill_at TEXT NOT NULL,
    PRIMARY KEY (category, user_id)
);
