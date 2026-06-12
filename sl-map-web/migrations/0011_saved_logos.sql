-- sqlfluff:dialect:sqlite
-- Saved logo images plus a link table from saved_renders so a render
-- carries a durable pointer to every logo image composited onto it.

PRAGMA foreign_keys = ON;

-- A persisted logo image. The binary bytes live on disk under
-- `<storage_dir>/logos/`; only the relative filename, MIME type and
-- intrinsic pixel dimensions are kept here so the library list and the
-- render worker can reason about a logo without reading the file.
-- Ownership follows the established Personal-or-Group XOR pattern used by
-- saved_notecards, saved_renders and saved_glw_data.
CREATE TABLE saved_logos (
    logo_id BLOB PRIMARY KEY NOT NULL,
    owner_user_id BLOB REFERENCES users (user_id) ON DELETE CASCADE,
    owner_group_id BLOB REFERENCES "groups" (group_id) ON DELETE CASCADE,
    -- Who uploaded the logo. ON DELETE SET NULL mirrors saved_notecards:
    -- audit history survives the uploader deleting their account.
    uploaded_by BLOB REFERENCES users (user_id) ON DELETE SET NULL,
    name TEXT NOT NULL,
    -- MIME type of the stored bytes: image/png, image/jpeg or image/webp.
    content_type TEXT NOT NULL,
    -- Relative filename under `<storage_dir>/logos/`.
    image_filename TEXT NOT NULL,
    -- Intrinsic pixel dimensions of the stored image, read at upload time
    -- so the placement-fit check does not have to decode the file.
    width INTEGER NOT NULL,
    height INTEGER NOT NULL,
    -- Size of the stored bytes, surfaced in the library list.
    byte_size INTEGER NOT NULL,
    created_at TEXT NOT NULL,
    CHECK ((owner_user_id IS NOT NULL) <> (owner_group_id IS NOT NULL))
);

CREATE INDEX saved_logos_user_idx ON saved_logos (
    owner_user_id, created_at DESC
);
CREATE INDEX saved_logos_group_idx ON saved_logos (
    owner_group_id, created_at DESC
);

-- Many-to-many link from a render to the logos composited onto it. A
-- render may carry several logos (one per placement slot), so this is a
-- link table rather than a single column like saved_renders.notecard_id /
-- glw_data_id.
--
-- ON DELETE CASCADE on render_id: deleting a render drops its link rows.
-- ON DELETE RESTRICT on logo_id: a logo referenced by any render cannot
-- be deleted until those renders are deleted first, mirroring the
-- saved_renders -> saved_notecards / saved_glw_data behaviour so
-- "Regenerate" always finds the logo it needs.
CREATE TABLE saved_render_logos (
    render_id BLOB NOT NULL,
    logo_id BLOB NOT NULL,
    PRIMARY KEY (render_id, logo_id),
    FOREIGN KEY (render_id) REFERENCES saved_renders (
        render_id
    ) ON DELETE CASCADE,
    FOREIGN KEY (logo_id) REFERENCES saved_logos (logo_id) ON DELETE RESTRICT
);

CREATE INDEX saved_render_logos_logo_idx ON saved_render_logos (logo_id);
