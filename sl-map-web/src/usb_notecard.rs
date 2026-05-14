//! Helpers for accepting a USB notecard from either an upload, a textarea,
//! or a reference to a previously-saved notecard by id.

use axum::extract::Multipart;
use sl_types::map::USBNotecard;
use uuid::Uuid;

use crate::error::Error;

/// Fields extracted from the multipart form for the notecard endpoints.
#[derive(Debug, Default)]
pub struct NotecardForm {
    /// the parsed USB notecard, when the form supplied the body inline
    /// (file upload or pasted text).
    pub notecard: Option<USBNotecard>,
    /// the id of a previously-saved notecard the caller wants to reuse.
    /// Mutually exclusive with an inline body; the handler is responsible
    /// for resolving the id to a body via the auth-checked lookup path.
    pub notecard_id: Option<Uuid>,
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

/// Parse a `multipart/form-data` body that may carry a USB notecard as a
/// file upload (`notecard`), as pasted text (`notecard_text`), or as a
/// reference to a saved notecard (`notecard_id`), plus optional border
/// fields. Exactly one of the three notecard sources must be supplied;
/// resolving the `notecard_id` to a body is the caller's job because it
/// requires the auth-checked DB lookup that this helper deliberately does
/// not know about.
///
/// # Errors
///
/// Returns a [`crate::error::Error`] if the multipart body fails to parse,
/// the notecard is not valid UTF-8, the notecard text cannot be parsed, a
/// numeric field is malformed, more than one of the three sources is
/// supplied, or none of them is.
pub async fn parse_notecard_form(mut multipart: Multipart) -> Result<NotecardForm, Error> {
    let mut form = NotecardForm::default();
    let mut notecard_text: Option<String> = None;
    let mut notecard_file: Option<String> = None;
    let mut notecard_id: Option<Uuid> = None;
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
            "notecard_id" => {
                let raw = field.text().await?;
                let trimmed = raw.trim();
                if !trimmed.is_empty() {
                    notecard_id = Some(
                        Uuid::parse_str(trimmed)
                            .map_err(|e| Error::BadRequest(format!("invalid notecard_id: {e}")))?,
                    );
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
    let inline = notecard_file.or(notecard_text);
    match (inline, notecard_id) {
        (Some(raw), None) => {
            form.notecard = Some(raw.parse()?);
        }
        (None, Some(id)) => {
            form.notecard_id = Some(id);
        }
        (Some(_), Some(_)) => {
            return Err(Error::BadRequest(
                "supply either `notecard_id` or notecard text/file, not both".to_owned(),
            ));
        }
        (None, None) => {
            return Err(Error::BadRequest(
                "supply a `notecard_id`, `notecard` file upload, or `notecard_text` field"
                    .to_owned(),
            ));
        }
    }
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
