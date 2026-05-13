//! Helpers for accepting a USB notecard from either an upload or a textarea.

use axum::extract::Multipart;
use sl_types::map::USBNotecard;

use crate::error::Error;

/// Fields extracted from the multipart form for the notecard endpoints.
#[derive(Debug, Default)]
pub struct NotecardForm {
    /// the parsed USB notecard.
    pub notecard: Option<USBNotecard>,
    /// uniform border in regions on every side (mutually exclusive with the
    /// per-side fields below).
    pub border_regions: Option<u16>,
    /// border added on the north (+y) side.
    pub border_north: Option<u16>,
    /// border added on the south (-y) side.
    pub border_south: Option<u16>,
    /// border added on the east (+x) side.
    pub border_east: Option<u16>,
    /// border added on the west (-x) side.
    pub border_west: Option<u16>,
}

impl NotecardForm {
    /// Resolve the four per-side border values, applying the `border_regions`
    /// shorthand if it was supplied.
    #[must_use]
    pub fn borders(&self) -> (u16, u16, u16, u16) {
        if let Some(b) = self.border_regions {
            (b, b, b, b)
        } else {
            (
                self.border_north.unwrap_or(0),
                self.border_south.unwrap_or(0),
                self.border_east.unwrap_or(0),
                self.border_west.unwrap_or(0),
            )
        }
    }
}

/// Parse a `multipart/form-data` body that may carry a USB notecard either
/// as a file upload (`notecard` field) or as pasted text (`notecard_text`
/// field), plus optional border fields.
///
/// # Errors
///
/// Returns a [`crate::error::Error`] if the multipart body fails to parse,
/// the notecard is not valid UTF-8, the notecard text cannot be parsed, a
/// numeric field is malformed, or no notecard field is supplied.
pub async fn parse_notecard_form(mut multipart: Multipart) -> Result<NotecardForm, Error> {
    let mut form = NotecardForm::default();
    let mut notecard_text: Option<String> = None;
    let mut notecard_file: Option<String> = None;
    while let Some(field) = multipart.next_field().await? {
        let Some(name) = field.name().map(str::to_owned) else {
            continue;
        };
        match name.as_str() {
            "notecard" => {
                let bytes = field.bytes().await?;
                if !bytes.is_empty() {
                    let text = String::from_utf8(bytes.to_vec())
                        .map_err(|e| Error::BadRequest(format!("notecard is not UTF-8: {e}")))?;
                    notecard_file = Some(text);
                }
            }
            "notecard_text" => {
                let text = field.text().await?;
                if !text.trim().is_empty() {
                    notecard_text = Some(text);
                }
            }
            "border_regions" => form.border_regions = parse_optional_u16(field.text().await?)?,
            "border_north" => form.border_north = parse_optional_u16(field.text().await?)?,
            "border_south" => form.border_south = parse_optional_u16(field.text().await?)?,
            "border_east" => form.border_east = parse_optional_u16(field.text().await?)?,
            "border_west" => form.border_west = parse_optional_u16(field.text().await?)?,
            _ => {}
        }
    }
    let raw = notecard_file.or(notecard_text).ok_or_else(|| {
        Error::BadRequest(
            "either a `notecard` file upload or a `notecard_text` field is required".to_owned(),
        )
    })?;
    form.notecard = Some(raw.parse()?);
    Ok(form)
}

/// Parse a possibly-empty textual u16. Returns `Ok(None)` for an empty
/// string so optional form fields can be left blank in the browser.
fn parse_optional_u16(s: String) -> Result<Option<u16>, Error> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    trimmed
        .parse::<u16>()
        .map(Some)
        .map_err(|e| Error::BadRequest(format!("invalid u16 `{trimmed}`: {e}")))
}
