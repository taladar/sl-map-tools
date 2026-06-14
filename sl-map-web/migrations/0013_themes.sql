-- sqlfluff:dialect:sqlite
-- Saved themes: a named bundle of the render page's presentation settings
-- (fill colours + enable state, region-overlay toggles + label font, GLW
-- style overrides + GLW font, default route colour) so a user can capture a
-- look once and re-apply it. Ownership follows the established
-- Personal-or-Group XOR pattern used by saved_notecards, saved_renders and
-- saved_glw_data.

PRAGMA foreign_keys = ON;

CREATE TABLE themes (
    theme_id BLOB PRIMARY KEY NOT NULL,
    owner_user_id BLOB REFERENCES users (user_id) ON DELETE CASCADE,
    owner_group_id BLOB REFERENCES "groups" (group_id) ON DELETE CASCADE,
    -- The avatar that created the theme. ON DELETE SET NULL (matching the
    -- audit columns elsewhere after migration 0008) so a group theme
    -- survives its creator deleting their account; a personal theme is
    -- removed first by the owner_user_id CASCADE.
    created_by BLOB REFERENCES users (user_id) ON DELETE SET NULL,
    name TEXT NOT NULL,
    -- The presentation settings as canonical JSON, deserialised back into
    -- `routes::themes::ThemeSettings` when applied. Carries its own
    -- `version` field for forward-compatible evolution.
    settings_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    -- Bumped on every rename / settings overwrite.
    updated_at TEXT NOT NULL,
    CHECK ((owner_user_id IS NOT NULL) <> (owner_group_id IS NOT NULL))
);

CREATE INDEX themes_user_idx ON themes (owner_user_id, created_at DESC);
CREATE INDEX themes_group_idx ON themes (owner_group_id, created_at DESC);

-- Theme names are unique within a scope (a user's personal library, or a
-- single group) so the picker never shows two same-named entries. Partial
-- indexes so each only covers the rows for its ownership kind; the unused
-- NULL owner column of the other kind is left out entirely.
CREATE UNIQUE INDEX themes_user_name_uniq ON themes (owner_user_id, name)
WHERE owner_user_id IS NOT NULL;

CREATE UNIQUE INDEX themes_group_name_uniq ON themes (owner_group_id, name)
WHERE owner_group_id IS NOT NULL;
