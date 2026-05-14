-- Cache the grid rectangle that a saved notecard's route occupies, so the
-- library UI can show the bounding region range without having to redo
-- the region-name resolution on every page load. The columns are
-- populated lazily — currently by `run_usb_notecard_job` the first time
-- a render runs against the notecard — and remain NULL for any notecard
-- that has never been rendered.
--
-- The route's start / end region names are not cached here because they
-- are cheap to read off the notecard body without any region-name
-- resolution (the body itself names them) and so do not benefit from
-- a column-side cache.
PRAGMA foreign_keys = ON;

ALTER TABLE saved_notecards ADD COLUMN lower_left_x INTEGER;
ALTER TABLE saved_notecards ADD COLUMN lower_left_y INTEGER;
ALTER TABLE saved_notecards ADD COLUMN upper_right_x INTEGER;
ALTER TABLE saved_notecards ADD COLUMN upper_right_y INTEGER;
