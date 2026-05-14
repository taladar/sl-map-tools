-- Store the rendered grid rectangle bounds on each saved render so the
-- library UI can show them without having to parse `settings_json` (which
-- only covers grid-rectangle renders) or compute them lazily from the
-- linked notecard (which would require region-name resolution).
--
-- The four columns are nullable because the bounds are not always known
-- at insert time:
--   * grid-rectangle renders set all four columns at insert
--   * usb-notecard renders compute the bounds in the background job once
--     the notecard has been parsed and region names resolved; the row is
--     updated in place at that point
--   * a usb-notecard render that fails before the rectangle is known
--     leaves the columns NULL, which is the correct "unknown" signal
PRAGMA foreign_keys = ON;

ALTER TABLE saved_renders ADD COLUMN lower_left_x INTEGER;
ALTER TABLE saved_renders ADD COLUMN lower_left_y INTEGER;
ALTER TABLE saved_renders ADD COLUMN upper_right_x INTEGER;
ALTER TABLE saved_renders ADD COLUMN upper_right_y INTEGER;
