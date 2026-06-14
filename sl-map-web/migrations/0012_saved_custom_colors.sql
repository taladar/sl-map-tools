-- sqlfluff:dialect:sqlite
-- Per-user palette of saved custom colours, shared across every colour
-- picker on the renderer page (route colour, missing-tile / missing-region
-- fills, the GLW style overrides, and the per-label text colour).
--
-- This generalises the single `users.route_color` preference (migration
-- 0009): instead of one remembered default, the user can build up a small
-- set of favourite shades that surface as preset swatches in *all* the
-- pickers via a shared `<datalist>`. Persisting on the account — rather
-- than in localStorage — keeps the palette following the user across
-- browsers and devices, the same rationale that justified `route_color`.
--
-- `(user_id, color)` is the primary key so a save is idempotent and a
-- colour cannot appear twice for the same user; `created_at` only orders
-- the palette for display. The format is canonical `#rrggbb`, enforced at
-- the API surface (SQLite has no native HEX-string check and the same
-- validation has to happen there anyway).

PRAGMA foreign_keys = ON;

CREATE TABLE saved_colors (
    -- the owning user; the palette is private to this account. ON DELETE
    -- CASCADE removes the palette when the account is deleted, mirroring
    -- sessions, memberships and personal-scope library items.
    user_id BLOB NOT NULL REFERENCES users (user_id) ON DELETE CASCADE,
    -- canonical `#rrggbb`, validated by the application before insert.
    color TEXT NOT NULL,
    -- orders the palette oldest-first for display.
    created_at TEXT NOT NULL,
    PRIMARY KEY (user_id, color)
);
