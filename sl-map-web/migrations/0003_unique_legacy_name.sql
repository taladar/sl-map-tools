-- sqlfluff:dialect:sqlite
-- Legacy names are one-to-one with avatar UUIDs in Linden Lab's account
-- system, so the column should never hold duplicates. Promote the index to
-- UNIQUE so any future insert path that violates this invariant fails loudly
-- instead of producing ambiguous logins by `legacy_name`.

DROP INDEX users_legacy_name_idx;
CREATE UNIQUE INDEX users_legacy_name_idx ON users (legacy_name);
