//! On-disk storage of saved render image bytes.
//!
//! Saved renders are stored as files under `<storage_dir>/renders/`. Each
//! file is named by the render's UUID plus a suffix that distinguishes the
//! primary image from the optional without-route variant. The DB row carries
//! just the relative filename so the actual storage location can change
//! without a schema migration.

use std::path::{Path, PathBuf};

use bytes::Bytes;
use uuid::Uuid;

use crate::error::Error;

/// Subdirectory under `storage_dir` where render image files live.
const RENDERS_SUBDIR: &str = "renders";

/// Suffix used for the primary rendered image file.
pub const IMAGE_SUFFIX: &str = "image";

/// Suffix used for the optional "without route" variant of a render.
pub const IMAGE_WITHOUT_ROUTE_SUFFIX: &str = "image-without-route";

/// Map an `image/...` MIME type to the file extension that should be used on
/// disk. Returns `bin` for anything unrecognised so the file is still
/// preserved and can be inspected manually.
#[must_use]
pub fn ext_for_content_type(content_type: &str) -> &'static str {
    match content_type {
        "image/png" => "png",
        "image/jpeg" => "jpg",
        _ => "bin",
    }
}

/// Compute the relative filename (relative to `<storage_dir>/renders/`) for a
/// render's image file given the render id, suffix, and extension.
#[must_use]
pub fn render_filename(render_id: Uuid, suffix: &str, ext: &str) -> String {
    format!("{render_id}-{suffix}.{ext}")
}

/// Compute the absolute path to a render file.
#[must_use]
pub fn render_path(storage_dir: &Path, filename: &str) -> PathBuf {
    storage_dir.join(RENDERS_SUBDIR).join(filename)
}

/// Create the on-startup subdirectory layout under `storage_dir`.
///
/// # Errors
///
/// Returns an [`Error::Io`] if the directories cannot be created.
pub fn ensure_layout(storage_dir: &Path) -> Result<(), Error> {
    fs_err::create_dir_all(storage_dir.join(RENDERS_SUBDIR))?;
    Ok(())
}

/// Write the rendered image bytes for a render to disk, returning the
/// filename that should be stored in the DB.
///
/// # Errors
///
/// Returns an [`Error::Io`] if the file cannot be written.
pub async fn write_render_file(
    storage_dir: &Path,
    render_id: Uuid,
    suffix: &str,
    ext: &str,
    bytes: Bytes,
) -> Result<String, Error> {
    let filename = render_filename(render_id, suffix, ext);
    let path = render_path(storage_dir, &filename);
    tokio::task::spawn_blocking(move || fs_err::write(&path, &bytes))
        .await
        .map_err(|err| Error::Io(std::io::Error::other(err)))??;
    Ok(filename)
}

/// Read the rendered image bytes back from disk.
///
/// # Errors
///
/// Returns [`Error::NotFound`] if the file does not exist, or [`Error::Io`]
/// for any other I/O failure.
pub async fn read_render_file(storage_dir: &Path, filename: &str) -> Result<Bytes, Error> {
    let path = render_path(storage_dir, filename);
    let filename_for_err = filename.to_owned();
    let bytes = tokio::task::spawn_blocking(move || fs_err::read(&path))
        .await
        .map_err(|err| Error::Io(std::io::Error::other(err)))?;
    match bytes {
        Ok(b) => Ok(Bytes::from(b)),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            Err(Error::NotFound(format!("render file `{filename_for_err}`")))
        }
        Err(err) => Err(Error::Io(err)),
    }
}

/// Best-effort delete of a single render file. A missing file is treated as
/// success (the row may have been deleted already). Other errors are
/// returned so the caller can decide whether to surface them or raise the
/// sweeper "dirty" flag for a later retry.
///
/// # Errors
///
/// Returns [`Error::Io`] for I/O failures other than `NotFound`.
pub fn try_delete_render_file(storage_dir: &Path, filename: &str) -> Result<(), Error> {
    let path = render_path(storage_dir, filename);
    match fs_err::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(Error::Io(err)),
    }
}

/// List the filenames currently present under the `renders/` subdirectory.
/// Used by the orphan sweeper.
///
/// # Errors
///
/// Returns [`Error::Io`] if the directory cannot be read.
pub fn list_render_files(storage_dir: &Path) -> Result<Vec<String>, Error> {
    let dir = storage_dir.join(RENDERS_SUBDIR);
    let mut out = Vec::new();
    let entries = fs_err::read_dir(&dir)?;
    for entry in entries {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }
        if let Some(name) = entry.file_name().to_str() {
            out.push(name.to_owned());
        }
    }
    Ok(out)
}

/// Extract the render UUID prefix from a filename produced by
/// [`render_filename`]. Returns `None` if the filename does not start with a
/// valid 36-character hyphenated UUID. Filenames are of the form
/// `{render_id}-{suffix}.{ext}`, where `render_id` itself contains four
/// hyphens, so we cannot just split on `-`.
#[must_use]
pub fn parse_render_id_from_filename(filename: &str) -> Option<Uuid> {
    // 36 = length of a hyphenated UUID, e.g. `00000000-0000-0000-0000-000000000000`.
    const UUID_LEN: usize = 36;
    let prefix: String = filename.chars().take(UUID_LEN).collect();
    if prefix.chars().count() != UUID_LEN {
        return None;
    }
    Uuid::parse_str(&prefix).ok()
}
