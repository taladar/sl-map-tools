-- sqlfluff:dialect:sqlite
-- Groups, group memberships/invitations, saved notecards/renders.

PRAGMA foreign_keys = ON;

-- A group bundles multiple users for the purpose of sharing a library of
-- saved notecards and rendered maps. The `created_by` column records the
-- avatar that originally created the group; ownership is governed by the
-- `group_memberships.role = 'owner'` rows, of which there must always be at
-- least one (enforced in the application layer when removing or demoting).
CREATE TABLE "groups" (
    group_id BLOB PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    created_by BLOB NOT NULL REFERENCES users (user_id) ON DELETE RESTRICT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX groups_name_idx ON "groups" (name);
CREATE INDEX groups_created_by_idx ON "groups" (created_by);

-- Membership of a user in a group. `role = 'owner'` grants full write access
-- to the group library and the ability to invite/remove/promote/demote
-- members; `role = 'member'` is read-only and only sees finished renders.
CREATE TABLE group_memberships (
    group_id BLOB NOT NULL REFERENCES "groups" (group_id) ON DELETE CASCADE,
    user_id BLOB NOT NULL REFERENCES users (user_id) ON DELETE CASCADE,
    role TEXT NOT NULL CHECK (role IN ('owner', 'member')),
    created_at TEXT NOT NULL,
    PRIMARY KEY (group_id, user_id)
);

CREATE INDEX group_memberships_user_idx ON group_memberships (user_id);
CREATE INDEX group_memberships_role_idx ON group_memberships (group_id, role);

-- A pending or resolved invitation to join a group with a given target role.
-- A unique partial index ensures that there is at most one pending invite
-- per (group, invitee) — resolved (accepted/rejected) rows stay for audit.
CREATE TABLE group_invitations (
    invitation_id BLOB PRIMARY KEY NOT NULL,
    group_id BLOB NOT NULL REFERENCES "groups" (group_id) ON DELETE CASCADE,
    invitee_id BLOB NOT NULL REFERENCES users (user_id) ON DELETE CASCADE,
    inviter_id BLOB NOT NULL REFERENCES users (user_id) ON DELETE CASCADE,
    target_role TEXT NOT NULL CHECK (target_role IN ('owner', 'member')),
    status TEXT NOT NULL CHECK (status IN ('pending', 'accepted', 'rejected'))
    DEFAULT 'pending',
    created_at TEXT NOT NULL,
    responded_at TEXT
);

CREATE UNIQUE INDEX group_invitations_pending_idx
ON group_invitations (group_id, invitee_id)
WHERE status = 'pending';
CREATE INDEX group_invitations_invitee_idx
ON group_invitations (invitee_id, status);

-- A saved USB notecard. Each row is owned by exactly one scope: either a
-- single user (personal library) or a group (group library). The notecard
-- body is stored inline because notecards are small (at most a few KB).
CREATE TABLE saved_notecards (
    notecard_id BLOB PRIMARY KEY NOT NULL,
    owner_user_id BLOB REFERENCES users (user_id) ON DELETE CASCADE,
    owner_group_id BLOB REFERENCES "groups" (group_id) ON DELETE CASCADE,
    uploaded_by BLOB NOT NULL REFERENCES users (user_id) ON DELETE RESTRICT,
    name TEXT NOT NULL,
    body TEXT NOT NULL,
    created_at TEXT NOT NULL,
    CHECK ((owner_user_id IS NOT NULL) <> (owner_group_id IS NOT NULL))
);

CREATE INDEX saved_notecards_user_idx
ON saved_notecards (owner_user_id, created_at DESC);
CREATE INDEX saved_notecards_group_idx
ON saved_notecards (owner_group_id, created_at DESC);
CREATE INDEX saved_notecards_uploaded_by_idx
ON saved_notecards (uploaded_by);

-- A saved render record. Created in `in_progress` state at submit time and
-- updated to `done` (with metadata + filenames) or `failed` (with the error
-- message) when the background render task finishes. The foreign key on
-- `notecard_id` uses ON DELETE RESTRICT so the user-facing rule "delete the
-- renders before you delete the notecard" is enforced at the DB layer.
CREATE TABLE saved_renders (
    render_id BLOB PRIMARY KEY NOT NULL,
    owner_user_id BLOB REFERENCES users (user_id) ON DELETE CASCADE,
    owner_group_id BLOB REFERENCES "groups" (group_id) ON DELETE CASCADE,
    created_by BLOB NOT NULL REFERENCES users (user_id) ON DELETE RESTRICT,
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
    CHECK ((owner_user_id IS NOT NULL) <> (owner_group_id IS NOT NULL))
);

CREATE INDEX saved_renders_user_idx
ON saved_renders (owner_user_id, created_at DESC);
CREATE INDEX saved_renders_group_idx
ON saved_renders (owner_group_id, created_at DESC);
CREATE INDEX saved_renders_notecard_idx ON saved_renders (notecard_id);
CREATE INDEX saved_renders_status_idx ON saved_renders (status);
