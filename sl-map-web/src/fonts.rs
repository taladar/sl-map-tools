//! Discovery of TrueType fonts under the configured
//! [`crate::config::Config::fonts_directory`].
//!
//! Scanned once at startup. The render workers then resolve a
//! client-supplied `font_id` (the filename basename) to a path under
//! that directory and load the bytes via `fs_err::read` +
//! `ab_glyph::FontVec::try_from_vec`. The library never bundles a font;
//! every font the user can pick comes from this directory.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Public metadata for a single font discovered at startup. Sent to
/// the browser by `GET /api/fonts`.
#[derive(Debug, Clone, serde::Serialize)]
pub struct FontInfo {
    /// Filename basename, e.g. `DejaVuSans.ttf`. Used as the opaque
    /// identifier in render-form requests. Never trust client input
    /// against the filesystem directly — always resolve via
    /// [`FontDirectory::path_for`].
    pub id: String,
    /// Display name. When the font's `name` table yields a usable name
    /// it is the embedded name with the file id in parentheses
    /// (`DejaVu Sans (DejaVuSans.ttf)`). Otherwise it falls back to the
    /// file stem with hyphens and underscores replaced by spaces
    /// (`noto-sans-mono.ttf` → `noto sans mono`).
    pub name: String,
}

/// Index of available fonts. Built once at startup, then cloned into
/// `AppState` as an `Arc<FontDirectory>`.
#[derive(Debug)]
pub struct FontDirectory {
    /// Absolute path the directory was opened at. Kept for resolving
    /// individual font paths and for diagnostic messages.
    root: PathBuf,
    /// `id` → discovered font. Sorted (BTreeMap) so iteration is
    /// deterministic when populating the UI.
    by_id: BTreeMap<String, FontEntry>,
}

/// One discovered font: its on-disk path plus the display label resolved
/// once at scan time (see [`FontInfo::name`]).
#[derive(Debug)]
struct FontEntry {
    /// absolute path to the `.ttf` file.
    path: PathBuf,
    /// pre-computed display label served by `GET /api/fonts`.
    display_name: String,
}

/// Errors that can occur while scanning a fonts directory.
#[derive(Debug, thiserror::Error)]
pub enum FontDirectoryError {
    /// the directory could not be opened.
    #[error("error opening fonts directory: {0}")]
    Io(#[from] std::io::Error),
    /// the directory exists but contains no `*.ttf` files.
    #[error(
        "fonts directory `{0}` contains no .ttf files; \
         add at least one font (DejaVuSans.ttf is checked in at the workspace root)"
    )]
    Empty(PathBuf),
}

impl FontDirectory {
    /// Scan `root` for `*.ttf` files and build the index. Returns an
    /// error if the directory cannot be opened or contains no fonts.
    ///
    /// # Errors
    ///
    /// Returns a [`FontDirectoryError`] when the directory is missing,
    /// unreadable, or has no `.ttf` entries.
    pub fn scan(root: PathBuf) -> Result<Self, FontDirectoryError> {
        let mut by_id: BTreeMap<String, FontEntry> = BTreeMap::new();
        for entry in fs_err::read_dir(&root)? {
            let entry = entry?;
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
                continue;
            };
            if !ext.eq_ignore_ascii_case("ttf") {
                continue;
            }
            let Some(id) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            // Resolve the display label once, here, so `GET /api/fonts`
            // never re-reads the file. A font that cannot be read or
            // parsed is still listed under its filename-derived label;
            // a truly unloadable font fails later at render via
            // `ab_glyph`, matching the pre-existing behaviour.
            let display_name = match fs_err::read(&path) {
                Ok(bytes) => embedded_name(&bytes)
                    .map_or_else(|| display_name_for(id), |name| format!("{name} ({id})")),
                Err(err) => {
                    tracing::warn!(font = id, %err, "could not read font for name extraction");
                    display_name_for(id)
                }
            };
            by_id.insert(id.to_owned(), FontEntry { path, display_name });
        }
        if by_id.is_empty() {
            return Err(FontDirectoryError::Empty(root));
        }
        Ok(Self { root, by_id })
    }

    /// Public list for `GET /api/fonts`.
    #[must_use]
    pub fn list(&self) -> Vec<FontInfo> {
        self.by_id
            .iter()
            .map(|(id, entry)| FontInfo {
                id: id.to_owned(),
                name: entry.display_name.clone(),
            })
            .collect()
    }

    /// Resolve a client-supplied `font_id` to the absolute on-disk
    /// path of the font file. Returns `None` if the id does not match
    /// a discovered font or if the id contains path-separator
    /// characters (defence in depth — the scan only inserts plain
    /// basenames, but client input is treated as untrusted).
    #[must_use]
    pub fn path_for(&self, font_id: &str) -> Option<&Path> {
        if font_id.is_empty()
            || font_id.contains('/')
            || font_id.contains('\\')
            || font_id.contains("..")
        {
            return None;
        }
        self.by_id.get(font_id).map(|entry| entry.path.as_path())
    }

    /// Root directory the scan was performed against; surfaced in
    /// error messages.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }
}

/// Convert a font filename (`DejaVuSans.ttf`) into a friendlier
/// display name (`DejaVu Sans`). The conversion is intentionally
/// simple: strip the extension, then replace `-` and `_` with spaces.
fn display_name_for(id: &str) -> String {
    let stem = id.rsplit_once('.').map_or(id, |(s, _)| s);
    stem.replace(['-', '_'], " ")
}

/// Extract a human-readable name from a TTF `name` table. Prefers the
/// Full font name (id 4), then the typographic family (id 16), then the
/// legacy family (id 1). Returns `None` when the file cannot be parsed or
/// has no decodable, non-empty record — the caller then falls back to the
/// filename via [`display_name_for`].
fn embedded_name(bytes: &[u8]) -> Option<String> {
    let face = ttf_parser::Face::parse(bytes, 0).ok()?;
    let names = face.names();
    // Name IDs from the OpenType `name` table, in display preference.
    for want in [4u16, 16, 1] {
        for i in 0..names.len() {
            let Some(name) = names.get(i) else { continue };
            if name.name_id != want || !name.is_unicode() {
                continue;
            }
            // `to_string` decodes Windows/Unicode UTF-16BE records and
            // returns `None` for encodings it cannot handle.
            if let Some(decoded) = name.to_string() {
                let trimmed = decoded.trim();
                if !trimmed.is_empty() {
                    return Some(trimmed.to_owned());
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod test {
    use super::*;
    use pretty_assertions::assert_eq;

    /// The font checked in at the workspace root, used to exercise the
    /// `name`-table extraction offline.
    const DEJAVU: &[u8] = include_bytes!("../../DejaVuSans.ttf");

    #[test]
    fn display_name_drops_extension_and_replaces_separators() {
        assert_eq!(display_name_for("DejaVuSans.ttf"), "DejaVuSans");
        assert_eq!(display_name_for("noto-sans-mono.ttf"), "noto sans mono");
        assert_eq!(display_name_for("source_code_pro.ttf"), "source code pro");
    }

    #[test]
    fn embedded_name_extracts_full_name() {
        assert_eq!(embedded_name(DEJAVU).as_deref(), Some("DejaVu Sans"));
    }

    #[test]
    fn embedded_name_rejects_garbage() {
        assert_eq!(embedded_name(b"not a font"), None);
    }

    #[test]
    fn list_uses_embedded_name_with_id() -> Result<(), Box<dyn std::error::Error>> {
        let tmp = tempfile::tempdir()?;
        fs_err::write(tmp.path().join("DejaVuSans.ttf"), DEJAVU)?;
        fs_err::write(tmp.path().join("bad.ttf"), b"not a font")?;
        let fd = FontDirectory::scan(tmp.path().to_owned())?;
        let fonts = fd.list();
        let dejavu = fonts
            .iter()
            .find(|f| f.id == "DejaVuSans.ttf")
            .ok_or("DejaVuSans.ttf missing from list")?;
        assert_eq!(dejavu.name, "DejaVu Sans (DejaVuSans.ttf)");
        let bad = fonts
            .iter()
            .find(|f| f.id == "bad.ttf")
            .ok_or("bad.ttf missing from list")?;
        assert_eq!(bad.name, "bad");
        Ok(())
    }

    #[test]
    fn path_for_rejects_path_traversal() -> Result<(), Box<dyn std::error::Error>> {
        let tmp = tempfile::tempdir()?;
        fs_err::write(tmp.path().join("one.ttf"), b"fake")?;
        let fd = FontDirectory::scan(tmp.path().to_owned())?;
        assert!(fd.path_for("one.ttf").is_some());
        assert!(fd.path_for("../one.ttf").is_none());
        assert!(fd.path_for("subdir/one.ttf").is_none());
        assert!(fd.path_for("missing.ttf").is_none());
        Ok(())
    }

    #[test]
    fn scan_empty_directory_fails() -> Result<(), Box<dyn std::error::Error>> {
        let tmp = tempfile::tempdir()?;
        let result = FontDirectory::scan(tmp.path().to_owned());
        assert!(
            matches!(result, Err(FontDirectoryError::Empty(_))),
            "expected Empty, got {result:?}",
        );
        Ok(())
    }
}
