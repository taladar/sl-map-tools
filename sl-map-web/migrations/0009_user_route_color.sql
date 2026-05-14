-- sqlfluff:dialect:sqlite
-- Per-user route-colour preference for the renderer page.
--
-- The picker's default is the hardcoded `#ff0000` in `index.html`, so
-- users who consistently prefer a different shade had to re-pick it for
-- every render. Persist it on the user row instead of in localStorage
-- so the preference follows the account across browsers and devices.
--
-- The format is canonical `#rrggbb` enforced by the application — the
-- column is plain TEXT here because SQLite has no native HEX-string
-- check and the same validation needs to happen at the API surface
-- anyway.

ALTER TABLE users ADD COLUMN route_color TEXT;
