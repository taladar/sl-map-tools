-- sqlfluff:dialect:sqlite
-- Initial schema for sl-map-web authentication.

PRAGMA foreign_keys = ON;

-- One row per registered avatar. The primary key is the 16-byte SL avatar
-- UUID, which is stable across legacy-name / username changes.
CREATE TABLE users (
    user_id BLOB PRIMARY KEY NOT NULL,
    legacy_name TEXT NOT NULL,
    username TEXT NOT NULL,
    password_hash TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE UNIQUE INDEX users_username_idx ON users (username);
CREATE INDEX users_legacy_name_idx ON users (legacy_name);

-- One-time, time-limited set-password / password-reset tokens. We store
-- only a SHA-256 of the raw token so that a DB read does not reveal a
-- working link.
CREATE TABLE set_password_tokens (
    token_hash BLOB PRIMARY KEY NOT NULL,
    user_id BLOB NOT NULL REFERENCES users (user_id) ON DELETE CASCADE,
    expires_at TEXT NOT NULL,
    used_at TEXT,
    created_at TEXT NOT NULL
);

CREATE INDEX set_password_tokens_user_id_idx ON set_password_tokens (user_id);
CREATE INDEX set_password_tokens_expires_at_idx ON set_password_tokens (
    expires_at
);

-- Server-side session store. The session cookie carries the raw session id
-- (random 32 bytes, signed via cookie::Key); the row stores only
-- SHA-256(raw) so that a DB read alone does not yield working session ids.
CREATE TABLE sessions (
    session_id_hash BLOB PRIMARY KEY NOT NULL,
    user_id BLOB NOT NULL REFERENCES users (user_id) ON DELETE CASCADE,
    expires_at TEXT NOT NULL,
    created_at TEXT NOT NULL,
    last_seen_at TEXT NOT NULL,
    client_ip TEXT
);

CREATE INDEX sessions_user_id_idx ON sessions (user_id);
CREATE INDEX sessions_expires_at_idx ON sessions (expires_at);
