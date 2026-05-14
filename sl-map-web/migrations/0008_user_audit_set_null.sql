-- sqlfluff:dialect:sqlite
-- Allow users to delete their own accounts.
--
-- The original schema had three columns referencing `users (user_id)` with
-- `ON DELETE RESTRICT NOT NULL`:
--
--   * `groups.created_by`              — historical/audit, never changes
--                                         after the row is inserted
--   * `saved_notecards.uploaded_by`    — historical/audit
--   * `saved_renders.created_by`       — historical/audit
--
-- Those constraints make every user who has ever created a group, uploaded
-- a group-scope notecard, or kicked off a group-scope render permanently
-- undeletable. The information is purely audit — current ownership for
-- groups is determined by `group_memberships.role = 'owner'`, not by
-- `created_by` — so the right thing on user deletion is to anonymise it,
-- not to refuse the delete or cascade-destroy the data.
--
-- Relax the three columns to `NULL` + `ON DELETE SET NULL`. SQLite cannot
-- change NOT NULL / FK in place, so we follow the standard 12-step recipe:
-- build the new table, copy the data, drop the old one, rename. Indexes
-- and triggers attached to the old tables are dropped with them; rebuild
-- them at the bottom.
--
-- `defer_foreign_keys = ON` defers FK checks until COMMIT so the
-- intermediate states (drop a parent table while a child still references
-- the old name) do not trip the FK check while the rebuild is in flight.

PRAGMA defer_foreign_keys = ON;

-- Drop the scope-match triggers up front. They live on
-- `saved_renders` and `saved_notecards`; dropping the parent tables
-- would otherwise leave SQLite probing them mid-rebuild against tables
-- that briefly do not exist, surfacing as
-- "error in trigger ...: no such table: main.saved_notecards" while
-- the migration is in flight.
DROP TRIGGER IF EXISTS saved_renders_notecard_scope_match_insert;
DROP TRIGGER IF EXISTS saved_renders_notecard_scope_match_update;
DROP TRIGGER IF EXISTS saved_notecards_scope_change_blocked;

-- ---------- groups ----------

CREATE TABLE groups_new (
    group_id BLOB PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    created_by BLOB REFERENCES users (user_id) ON DELETE SET NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

INSERT INTO groups_new (group_id, name, created_by, created_at, updated_at)
SELECT
    group_id,
    name,
    created_by,
    created_at,
    updated_at
FROM "groups";

DROP TABLE "groups";
ALTER TABLE groups_new RENAME TO "groups";

CREATE INDEX groups_name_idx ON "groups" (name);
CREATE INDEX groups_created_by_idx ON "groups" (created_by);

-- ---------- saved_notecards ----------

CREATE TABLE saved_notecards_new (
    notecard_id BLOB PRIMARY KEY NOT NULL,
    owner_user_id BLOB REFERENCES users (user_id) ON DELETE CASCADE,
    owner_group_id BLOB REFERENCES "groups" (group_id) ON DELETE CASCADE,
    uploaded_by BLOB REFERENCES users (user_id) ON DELETE SET NULL,
    name TEXT NOT NULL,
    body TEXT NOT NULL,
    created_at TEXT NOT NULL,
    lower_left_x INTEGER,
    lower_left_y INTEGER,
    upper_right_x INTEGER,
    upper_right_y INTEGER,
    CHECK ((owner_user_id IS NOT NULL) <> (owner_group_id IS NOT NULL))
);

INSERT INTO saved_notecards_new (
    notecard_id, owner_user_id, owner_group_id, uploaded_by,
    name, body, created_at,
    lower_left_x, lower_left_y, upper_right_x, upper_right_y
)
SELECT
    notecard_id,
    owner_user_id,
    owner_group_id,
    uploaded_by,
    name,
    body,
    created_at,
    lower_left_x,
    lower_left_y,
    upper_right_x,
    upper_right_y
FROM saved_notecards;

DROP TABLE saved_notecards;
ALTER TABLE saved_notecards_new RENAME TO saved_notecards;

CREATE INDEX saved_notecards_user_idx
ON saved_notecards (owner_user_id, created_at DESC);
CREATE INDEX saved_notecards_group_idx
ON saved_notecards (owner_group_id, created_at DESC);
CREATE INDEX saved_notecards_uploaded_by_idx
ON saved_notecards (uploaded_by);

-- ---------- saved_renders ----------

CREATE TABLE saved_renders_new (
    render_id BLOB PRIMARY KEY NOT NULL,
    owner_user_id BLOB REFERENCES users (user_id) ON DELETE CASCADE,
    owner_group_id BLOB REFERENCES "groups" (group_id) ON DELETE CASCADE,
    created_by BLOB REFERENCES users (user_id) ON DELETE SET NULL,
    notecard_id BLOB REFERENCES saved_notecards (
        notecard_id
    ) ON DELETE RESTRICT,
    kind TEXT NOT NULL CHECK (kind IN ('grid_rectangle', 'usb_notecard')),
    status TEXT NOT NULL CHECK (status IN ('in_progress', 'done', 'failed'))
    DEFAULT 'in_progress',
    error_message TEXT,
    settings_json TEXT NOT NULL,
    metadata_json TEXT,
    content_type TEXT,
    image_filename TEXT,
    image_without_route_filename TEXT,
    created_at TEXT NOT NULL,
    finished_at TEXT,
    lower_left_x INTEGER,
    lower_left_y INTEGER,
    upper_right_x INTEGER,
    upper_right_y INTEGER,
    CHECK ((owner_user_id IS NOT NULL) <> (owner_group_id IS NOT NULL))
);

INSERT INTO saved_renders_new (
    render_id, owner_user_id, owner_group_id, created_by, notecard_id,
    kind, status, error_message, settings_json, metadata_json,
    content_type, image_filename, image_without_route_filename,
    created_at, finished_at,
    lower_left_x, lower_left_y, upper_right_x, upper_right_y
)
SELECT
    render_id,
    owner_user_id,
    owner_group_id,
    created_by,
    notecard_id,
    kind,
    status,
    error_message,
    settings_json,
    metadata_json,
    content_type,
    image_filename,
    image_without_route_filename,
    created_at,
    finished_at,
    lower_left_x,
    lower_left_y,
    upper_right_x,
    upper_right_y
FROM saved_renders;

DROP TABLE saved_renders;
ALTER TABLE saved_renders_new RENAME TO saved_renders;

CREATE INDEX saved_renders_user_idx
ON saved_renders (owner_user_id, created_at DESC);
CREATE INDEX saved_renders_group_idx
ON saved_renders (owner_group_id, created_at DESC);
CREATE INDEX saved_renders_notecard_idx ON saved_renders (notecard_id);
CREATE INDEX saved_renders_status_idx ON saved_renders (status);

-- ---------- triggers from 0005 (lost when their parent table dropped) ---

CREATE TRIGGER saved_renders_notecard_scope_match_insert
BEFORE INSERT ON saved_renders
FOR EACH ROW
WHEN new.notecard_id IS NOT NULL
BEGIN
    SELECT RAISE(ABORT, 'render and notecard must share the same scope')
    WHERE NOT EXISTS (
        SELECT 1 FROM saved_notecards
        WHERE
            saved_notecards.notecard_id = new.notecard_id
            AND saved_notecards.owner_user_id IS new.owner_user_id
            AND saved_notecards.owner_group_id IS new.owner_group_id
    );
END;

CREATE TRIGGER saved_renders_notecard_scope_match_update
BEFORE UPDATE OF notecard_id, owner_user_id, owner_group_id ON saved_renders
FOR EACH ROW
WHEN new.notecard_id IS NOT NULL
BEGIN
    SELECT RAISE(ABORT, 'render and notecard must share the same scope')
    WHERE NOT EXISTS (
        SELECT 1 FROM saved_notecards
        WHERE
            saved_notecards.notecard_id = new.notecard_id
            AND saved_notecards.owner_user_id IS new.owner_user_id
            AND saved_notecards.owner_group_id IS new.owner_group_id
    );
END;

CREATE TRIGGER saved_notecards_scope_change_blocked
BEFORE UPDATE OF owner_user_id, owner_group_id ON saved_notecards
FOR EACH ROW
WHEN (
    new.owner_user_id IS NOT old.owner_user_id
    OR new.owner_group_id IS NOT old.owner_group_id
)
AND EXISTS (
    SELECT 1 FROM saved_renders
    WHERE saved_renders.notecard_id = old.notecard_id
)
BEGIN
    SELECT RAISE(ABORT, 'cannot re-scope a notecard referenced by a render');
END;
