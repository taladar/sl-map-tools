-- sqlfluff:dialect:sqlite
-- Normalise stored UTC timestamps from the "+00:00" suffix written by the
-- former chrono-based sqlx encoding to the "Z" suffix written by the
-- time-based encoding (RFC 3339), so text comparisons and ORDER BY on these
-- columns see a single consistent format. Both forms decode with either
-- library, so older binaries can still read the rewritten rows.

UPDATE group_invitations
SET created_at = substr(created_at, 1, length(created_at) - 6) || 'Z'
WHERE created_at LIKE '%+00:00';

UPDATE group_invitations
SET responded_at = substr(responded_at, 1, length(responded_at) - 6) || 'Z'
WHERE responded_at LIKE '%+00:00';

UPDATE group_memberships
SET created_at = substr(created_at, 1, length(created_at) - 6) || 'Z'
WHERE created_at LIKE '%+00:00';

UPDATE "groups"
SET created_at = substr(created_at, 1, length(created_at) - 6) || 'Z'
WHERE created_at LIKE '%+00:00';

UPDATE "groups"
SET updated_at = substr(updated_at, 1, length(updated_at) - 6) || 'Z'
WHERE updated_at LIKE '%+00:00';

UPDATE rate_buckets
SET
    last_refill_at
    = substr(last_refill_at, 1, length(last_refill_at) - 6) || 'Z'
WHERE last_refill_at LIKE '%+00:00';

UPDATE saved_colors
SET created_at = substr(created_at, 1, length(created_at) - 6) || 'Z'
WHERE created_at LIKE '%+00:00';

UPDATE saved_glw_data
SET created_at = substr(created_at, 1, length(created_at) - 6) || 'Z'
WHERE created_at LIKE '%+00:00';

UPDATE saved_glw_data
SET fetched_at = substr(fetched_at, 1, length(fetched_at) - 6) || 'Z'
WHERE fetched_at LIKE '%+00:00';

UPDATE saved_logos
SET created_at = substr(created_at, 1, length(created_at) - 6) || 'Z'
WHERE created_at LIKE '%+00:00';

UPDATE saved_notecards
SET created_at = substr(created_at, 1, length(created_at) - 6) || 'Z'
WHERE created_at LIKE '%+00:00';

UPDATE saved_renders
SET created_at = substr(created_at, 1, length(created_at) - 6) || 'Z'
WHERE created_at LIKE '%+00:00';

UPDATE saved_renders
SET finished_at = substr(finished_at, 1, length(finished_at) - 6) || 'Z'
WHERE finished_at LIKE '%+00:00';

UPDATE sessions
SET created_at = substr(created_at, 1, length(created_at) - 6) || 'Z'
WHERE created_at LIKE '%+00:00';

UPDATE sessions
SET expires_at = substr(expires_at, 1, length(expires_at) - 6) || 'Z'
WHERE expires_at LIKE '%+00:00';

UPDATE sessions
SET last_seen_at = substr(last_seen_at, 1, length(last_seen_at) - 6) || 'Z'
WHERE last_seen_at LIKE '%+00:00';

UPDATE set_password_tokens
SET created_at = substr(created_at, 1, length(created_at) - 6) || 'Z'
WHERE created_at LIKE '%+00:00';

UPDATE set_password_tokens
SET expires_at = substr(expires_at, 1, length(expires_at) - 6) || 'Z'
WHERE expires_at LIKE '%+00:00';

UPDATE set_password_tokens
SET used_at = substr(used_at, 1, length(used_at) - 6) || 'Z'
WHERE used_at LIKE '%+00:00';

UPDATE themes
SET created_at = substr(created_at, 1, length(created_at) - 6) || 'Z'
WHERE created_at LIKE '%+00:00';

UPDATE themes
SET updated_at = substr(updated_at, 1, length(updated_at) - 6) || 'Z'
WHERE updated_at LIKE '%+00:00';

UPDATE users
SET created_at = substr(created_at, 1, length(created_at) - 6) || 'Z'
WHERE created_at LIKE '%+00:00';

UPDATE users
SET updated_at = substr(updated_at, 1, length(updated_at) - 6) || 'Z'
WHERE updated_at LIKE '%+00:00';
