-- Enforce at the DB level that every `saved_renders.notecard_id` reference
-- points at a notecard in the same scope as the render itself. Without this,
-- cross-scope references (render owned by group B, notecard owned by group
-- A) prevented group A from ever being deleted: the cascade would queue NC
-- for deletion, but the render in B holds an ON DELETE RESTRICT FK to NC
-- and the cascade aborts.
--
-- The application also auto-copies the notecard into the render's scope on
-- submit, so users never see a rejection in the normal flow. The triggers
-- below are the durable guarantee that no future code path (manual SQL,
-- admin tool, future endpoint, migration bug) can mint a cross-scope row.
--
-- `IS` (not `=`) is SQLite's NULL-safe equality. The schema guarantees that
-- exactly one of `owner_user_id` / `owner_group_id` is NULL on every row,
-- so `=` would never match across rows that share a Personal owner. `IS`
-- treats `NULL IS NULL` as true, which is what the invariant requires.

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

-- Defence in depth: the app has no "re-scope a notecard" operation today,
-- but if one is ever added, a re-scope while a render still references the
-- notecard would silently break the invariant the two triggers above
-- guard. Block it at write time.
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
