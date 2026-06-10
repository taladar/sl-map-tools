-- sqlfluff:dialect:sqlite
-- Saved GLW (GlobalWind) event data plus a foreign-key reference from
-- saved_renders so a render carries a durable pointer to the GLW event
-- that produced it.

PRAGMA foreign_keys = ON;

-- A persisted GLW event. The payload is the full JSON document returned by
-- (or pasted in to substitute for) the GLW server, stored verbatim so it can
-- be deserialised back into `sl_glw::GlwEvent` at render time. Ownership
-- follows the established Personal-or-Group XOR pattern used by
-- saved_notecards and saved_renders.
CREATE TABLE saved_glw_data (
    glw_data_id BLOB PRIMARY KEY NOT NULL,
    owner_user_id BLOB REFERENCES users (user_id) ON DELETE CASCADE,
    owner_group_id BLOB REFERENCES "groups" (group_id) ON DELETE CASCADE,
    created_by BLOB NOT NULL REFERENCES users (user_id) ON DELETE RESTRICT,
    name TEXT NOT NULL,
    -- Where the event originally came from, so the library list can
    -- distinguish "fetched by id", "fetched by key", and "pasted JSON",
    -- and so a future re-fetch action knows what to send back to the
    -- server.
    source_kind TEXT NOT NULL CHECK (
        source_kind IN ('event_id', 'event_key', 'pasted_json')
    ),
    source_event_id INTEGER,
    source_event_key TEXT,
    -- The resolved GLW event as canonical JSON, parsed back into
    -- `sl_glw::GlwEvent` at render time.
    payload_json TEXT NOT NULL,
    -- Convenience fields surfaced in the library list without having
    -- to deserialise payload_json just to render a row.
    event_id INTEGER,
    event_key TEXT,
    event_name TEXT,
    fetched_at TEXT NOT NULL,
    created_at TEXT NOT NULL,
    CHECK ((owner_user_id IS NOT NULL) <> (owner_group_id IS NOT NULL))
);

CREATE INDEX saved_glw_data_user_idx ON saved_glw_data (
    owner_user_id, created_at DESC
);
CREATE INDEX saved_glw_data_group_idx ON saved_glw_data (
    owner_group_id, created_at DESC
);

-- "Show me every fetch of this event id (or key), most recent first" —
-- supports the library filtering the user asked for. Partial indices so
-- the irrelevant source_kind rows do not bloat the index.
CREATE INDEX saved_glw_data_source_event_id_idx ON saved_glw_data (
    source_event_id, fetched_at DESC
)
WHERE source_event_id IS NOT NULL;

CREATE INDEX saved_glw_data_source_event_key_idx ON saved_glw_data (
    source_event_key, fetched_at DESC
)
WHERE source_event_key IS NOT NULL;

-- Renders that used a GLW overlay reference the saved_glw_data row that
-- produced them. ON DELETE RESTRICT mirrors the saved_notecards link: a
-- referenced GLW row cannot be deleted until every render that uses it is
-- deleted first. Renders without GLW have glw_data_id = NULL.
ALTER TABLE saved_renders
ADD COLUMN glw_data_id BLOB
REFERENCES saved_glw_data (glw_data_id) ON DELETE RESTRICT;

CREATE INDEX saved_renders_glw_data_idx ON saved_renders (glw_data_id);
