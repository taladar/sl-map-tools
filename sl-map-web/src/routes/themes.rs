//! HTTP handlers for saved themes.
//!
//! A *theme* is a named bundle of the render page's presentation settings —
//! the fill colours and their enable toggles, the region-overlay toggles
//! and label font, the GLW style overrides and GLW font, and the default
//! route colour. Users save the current look once and re-apply it later.
//!
//! Ownership follows the established Personal-or-Group XOR pattern; the
//! permission gates live in [`crate::library`] alongside the other library
//! item types. Group themes are writable by group owners only; every member
//! can list and apply them.

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode as ReqwestStatusCode;
use axum::response::{IntoResponse as _, Response};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::auth::{CurrentUser, uuid_from_bytes};
use crate::error::{self, Error};
use crate::library::{self, Destination, ThemeRow, is_canonical_hex_color};
use crate::routes::render::GlwStyleOverrides;
use crate::state::AppState;

/// Current `settings_json` schema version. Bumped if the shape of
/// [`ThemeSettings`] ever changes incompatibly.
const THEME_SETTINGS_VERSION: u32 = 1;

/// Default for the serde `version` field on older / hand-written payloads.
const fn default_version() -> u32 {
    THEME_SETTINGS_VERSION
}

/// The presentation settings captured by a saved theme. Serialised to the
/// `themes.settings_json` column and applied back onto the render form by
/// the frontend. Every field is `#[serde(default)]` so a payload written by
/// an older client (missing a field) still deserialises cleanly.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "this is a flat presentation-settings record; each bool maps directly to one independent checkbox in the render form"
)]
pub struct ThemeSettings {
    /// payload schema version (see `THEME_SETTINGS_VERSION`).
    #[serde(default = "default_version")]
    pub version: u32,
    /// whether the "fill missing map tiles" option is enabled.
    #[serde(default)]
    pub missing_map_tile_enabled: bool,
    /// fill colour for missing map tiles, as `#rrggbb`.
    #[serde(default)]
    pub missing_map_tile_color: Option<String>,
    /// whether the "fill missing regions" option is enabled.
    #[serde(default)]
    pub missing_region_enabled: bool,
    /// fill colour for missing regions, as `#rrggbb`.
    #[serde(default)]
    pub missing_region_color: Option<String>,
    /// draw a hairline rectangle around each region.
    #[serde(default)]
    pub draw_region_rectangles: bool,
    /// draw each region's name.
    #[serde(default)]
    pub draw_region_names: bool,
    /// draw each region's grid coordinates.
    #[serde(default)]
    pub draw_region_coordinates: bool,
    /// font id used for the region name / coordinate overlay.
    #[serde(default)]
    pub region_label_font_id: Option<String>,
    /// GLW style overrides (margin band + the seven GLW colours). Reuses
    /// the render module's struct so the JS maps straight onto it.
    #[serde(default)]
    pub glw_style: GlwStyleOverrides,
    /// font id used for GLW labels and the legend.
    #[serde(default)]
    pub glw_font_id: Option<String>,
    /// default route polyline colour, as `#rrggbb`.
    #[serde(default)]
    pub route_color: Option<String>,
}

impl ThemeSettings {
    /// Validate every colour field is canonical `#rrggbb`. Font ids are
    /// stored as-is (the render path already tolerates a missing font).
    ///
    /// # Errors
    ///
    /// Returns [`Error::BadRequest`] for any malformed colour.
    fn validate(&self) -> Result<(), Error> {
        let colors = [
            ("missing_map_tile_color", &self.missing_map_tile_color),
            ("missing_region_color", &self.missing_region_color),
            (
                "glw_style.area_outline_color",
                &self.glw_style.area_outline_color,
            ),
            (
                "glw_style.circle_outline_color",
                &self.glw_style.circle_outline_color,
            ),
            (
                "glw_style.margin_outline_color",
                &self.glw_style.margin_outline_color,
            ),
            ("glw_style.wind_color", &self.glw_style.wind_color),
            ("glw_style.current_color", &self.glw_style.current_color),
            ("glw_style.wave_color", &self.glw_style.wave_color),
            ("glw_style.label_color", &self.glw_style.label_color),
            ("route_color", &self.route_color),
        ];
        for (field, value) in colors {
            if let Some(c) = value
                && !is_canonical_hex_color(c)
            {
                return Err(Error::BadRequest(format!(
                    "{field} must be canonical `#rrggbb`, got {c:?}"
                )));
            }
        }
        Ok(())
    }
}

/// Public, serialisable record of a saved theme.
#[derive(Debug, Serialize)]
pub struct ThemeView {
    /// the theme id.
    pub theme_id: Uuid,
    /// the destination the theme belongs to.
    pub destination: Destination,
    /// the avatar that created the theme, or `None` if the account is
    /// since deleted.
    pub created_by: Option<Uuid>,
    /// the creator's username, if the account still exists.
    pub created_by_username: Option<String>,
    /// the creator's legacy name, if the account still exists.
    pub created_by_legacy_name: Option<String>,
    /// the human-supplied display name.
    pub name: String,
    /// the presentation settings.
    pub settings: ThemeSettings,
    /// when the theme was created.
    pub created_at: DateTime<Utc>,
    /// when the theme was last renamed or its settings overwritten.
    pub updated_at: DateTime<Utc>,
}

/// Query parameters for `GET /api/themes`.
#[derive(Debug, Deserialize)]
pub struct ListQuery {
    /// `"personal"` or `"group:<uuid>"`.
    pub scope: String,
}

/// Response shape for `GET /api/themes`.
#[derive(Debug, Serialize)]
pub struct ListThemesResponse {
    /// matching themes, newest first.
    pub themes: Vec<ThemeView>,
}

/// Response shape for a single theme.
#[derive(Debug, Serialize)]
pub struct ThemeResponse {
    /// the requested theme.
    pub theme: ThemeView,
}

/// Body for `POST /api/themes`.
#[derive(Debug, Deserialize)]
pub struct CreateThemeRequest {
    /// destination scope: `"personal"` or `"group:<uuid>"`.
    pub scope: String,
    /// the display name for the new theme.
    pub name: String,
    /// the presentation settings to store.
    pub settings: ThemeSettings,
}

/// Body for `PATCH /api/themes/{id}`. Either or both fields may be set;
/// `name` renames the theme, `settings` overwrites the stored settings.
#[derive(Debug, Deserialize)]
pub struct UpdateThemeRequest {
    /// new display name, if renaming.
    #[serde(default)]
    pub name: Option<String>,
    /// new settings, if overwriting.
    #[serde(default)]
    pub settings: Option<ThemeSettings>,
}

/// Parse a theme row's `settings_json` into [`ThemeSettings`].
fn parse_settings(settings_json: &str) -> Result<ThemeSettings, Error> {
    serde_json::from_str(settings_json).map_err(|err| {
        tracing::error!("theme settings_json parse failed: {err}");
        Error::Database
    })
}

/// Build a [`ThemeView`] from a fetched row and the resolved creator
/// display-name pair.
fn build_view(
    row: ThemeRow,
    destination: Destination,
    created_by_username: Option<String>,
    created_by_legacy_name: Option<String>,
) -> Result<ThemeView, Error> {
    let settings = parse_settings(&row.settings_json)?;
    Ok(ThemeView {
        theme_id: row.theme_id,
        destination,
        created_by: row.created_by,
        created_by_username,
        created_by_legacy_name,
        name: row.name,
        settings,
        created_at: row.created_at,
        updated_at: row.updated_at,
    })
}

/// Look up a user's display fields for view-building. Mirrors the helper
/// in [`crate::routes::glw`].
async fn lookup_user_names(state: &AppState, user_id: Uuid) -> Result<(String, String), Error> {
    let row: Option<(String, String)> =
        sqlx::query_as("SELECT username, legacy_name FROM users WHERE user_id = ?1")
            .bind(user_id.as_bytes().to_vec())
            .fetch_optional(&state.db)
            .await
            .map_err(|err| {
                tracing::error!("user name lookup failed: {err}");
                Error::Database
            })?;
    row.ok_or_else(|| Error::NotFound(format!("user {user_id}")))
}

/// Resolve the creator display-name pair for a row, swallowing a missing
/// account into `(None, None)`.
async fn resolve_creator(
    state: &AppState,
    created_by: Option<Uuid>,
) -> (Option<String>, Option<String>) {
    match created_by {
        Some(id) => match lookup_user_names(state, id).await {
            Ok((u, l)) => (Some(u), Some(l)),
            Err(_) => (None, None),
        },
        None => (None, None),
    }
}

/// Row shape returned by the listing query.
#[derive(sqlx::FromRow)]
struct ThemeListRow {
    /// raw bytes of `themes.theme_id`.
    theme_id: Vec<u8>,
    /// raw bytes of `themes.owner_user_id`, if set.
    owner_user_id: Option<Vec<u8>>,
    /// raw bytes of `themes.owner_group_id`, if set.
    owner_group_id: Option<Vec<u8>>,
    /// raw bytes of the creating user's id, if still set.
    created_by: Option<Vec<u8>>,
    /// the creating user's `username`.
    created_by_username: Option<String>,
    /// the creating user's `legacy_name`.
    created_by_legacy_name: Option<String>,
    /// display name.
    name: String,
    /// the presentation settings as JSON.
    settings_json: String,
    /// row creation timestamp.
    created_at: DateTime<Utc>,
    /// last-modified timestamp.
    updated_at: DateTime<Utc>,
}

/// `GET /api/themes?scope=…` — list themes in a scope. Personal lists the
/// caller's own themes; a group lists that group's themes (visible to any
/// member).
///
/// # Errors
///
/// Returns [`Error::BadRequest`] for an invalid scope; [`Error::Forbidden`]
/// if the user is not allowed to view the scope.
pub async fn list(
    user: CurrentUser,
    State(state): State<AppState>,
    Query(query): Query<ListQuery>,
) -> Result<Json<ListThemesResponse>, Error> {
    let destination = Destination::parse(&query.scope)?;
    library::assert_can_view(&state.db, user.user_id, destination).await?;

    // Both queries are built from string literals only; all user-supplied
    // values are passed via bind parameters, so there is no injection risk.
    let rows: Vec<ThemeListRow> = match destination {
        Destination::Personal => {
            sqlx::query_as(
                "SELECT t.theme_id, t.owner_user_id, t.owner_group_id, t.created_by, \
                        u.username AS created_by_username, u.legacy_name AS created_by_legacy_name, \
                        t.name, t.settings_json, t.created_at, t.updated_at \
                 FROM themes AS t \
                 LEFT JOIN users AS u ON u.user_id = t.created_by \
                 WHERE t.owner_user_id = ?1 \
                 ORDER BY t.created_at DESC",
            )
            .bind(user.user_id.as_bytes().to_vec())
            .fetch_all(&state.db)
            .await
        }
        Destination::Group { group_id } => {
            // The JOIN against group_memberships enforces visibility at the
            // SQL layer, matching the saved_glw_data list query.
            sqlx::query_as(
                "SELECT t.theme_id, t.owner_user_id, t.owner_group_id, t.created_by, \
                        u.username AS created_by_username, u.legacy_name AS created_by_legacy_name, \
                        t.name, t.settings_json, t.created_at, t.updated_at \
                 FROM themes AS t \
                 LEFT JOIN users AS u ON u.user_id = t.created_by \
                 JOIN group_memberships AS gm \
                   ON gm.group_id = t.owner_group_id AND gm.user_id = ?2 \
                 WHERE t.owner_group_id = ?1 \
                 ORDER BY t.created_at DESC",
            )
            .bind(group_id.as_bytes().to_vec())
            .bind(user.user_id.as_bytes().to_vec())
            .fetch_all(&state.db)
            .await
        }
    }
    .map_err(|err| {
        tracing::error!("list themes failed: {err}");
        Error::Database
    })?;

    let mut themes = Vec::with_capacity(rows.len());
    for row in rows {
        let theme_id = uuid_from_bytes(&row.theme_id).ok_or_else(|| {
            tracing::error!("bad theme uuid");
            Error::Database
        })?;
        let row_dest = library::destination_from_columns(row.owner_user_id, row.owner_group_id)?;
        let created_by = row
            .created_by
            .as_deref()
            .map(uuid_from_bytes)
            .map(|opt| {
                opt.ok_or_else(|| {
                    tracing::error!("bad created_by uuid in themes");
                    Error::Database
                })
            })
            .transpose()?;
        let settings = parse_settings(&row.settings_json)?;
        themes.push(ThemeView {
            theme_id,
            destination: row_dest,
            created_by,
            created_by_username: row.created_by_username,
            created_by_legacy_name: row.created_by_legacy_name,
            name: row.name,
            settings,
            created_at: row.created_at,
            updated_at: row.updated_at,
        });
    }
    Ok(Json(ListThemesResponse { themes }))
}

/// `POST /api/themes` — create a new theme in a scope. Personal scope is
/// always allowed; group scope requires owner membership.
///
/// # Errors
///
/// Returns [`Error::BadRequest`] / [`Error::Forbidden`].
pub async fn create(
    user: CurrentUser,
    State(state): State<AppState>,
    Json(body): Json<CreateThemeRequest>,
) -> Result<Json<ThemeResponse>, Error> {
    let destination = Destination::parse(&body.scope)?;
    library::assert_can_write(&state.db, user.user_id, destination).await?;
    let name = library::sanitise_display_name(&body.name, "theme name")?;
    body.settings.validate()?;

    let settings_json = serde_json::to_string(&body.settings).map_err(|err| {
        tracing::error!("theme settings serialise failed: {err}");
        Error::Database
    })?;
    let theme_id = Uuid::new_v4();
    let now = Utc::now();
    let (owner_user, owner_group) = match destination {
        Destination::Personal => (Some(user.user_id.as_bytes().to_vec()), None),
        Destination::Group { group_id } => (None, Some(group_id.as_bytes().to_vec())),
    };
    sqlx::query(
        "INSERT INTO themes \
            (theme_id, owner_user_id, owner_group_id, created_by, name, \
             settings_json, created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)",
    )
    .bind(theme_id.as_bytes().to_vec())
    .bind(owner_user)
    .bind(owner_group)
    .bind(user.user_id.as_bytes().to_vec())
    .bind(&name)
    .bind(&settings_json)
    .bind(now)
    .execute(&state.db)
    .await
    .map_err(|err| {
        if error::is_unique_violation(&err) {
            return Error::BadRequest(format!(
                "a theme named {name:?} already exists in this scope; pick a different name"
            ));
        }
        tracing::error!("insert theme failed: {err}");
        Error::Database
    })?;

    let view = ThemeView {
        theme_id,
        destination,
        created_by: Some(user.user_id),
        created_by_username: Some(user.username.clone()),
        created_by_legacy_name: Some(user.legacy_name.clone()),
        name,
        settings: body.settings,
        created_at: now,
        updated_at: now,
    };
    Ok(Json(ThemeResponse { theme: view }))
}

/// `GET /api/themes/{id}` — fetch a single theme. Personal owner or any
/// member of the owning group.
///
/// # Errors
///
/// Returns [`Error::NotFound`] if the theme doesn't exist or is invisible.
pub async fn get(
    user: CurrentUser,
    State(state): State<AppState>,
    Path(theme_id): Path<Uuid>,
) -> Result<Json<ThemeResponse>, Error> {
    let row = library::assert_can_read_theme(&state.db, user.user_id, theme_id).await?;
    let destination =
        library::destination_from_columns(row.owner_user_id.clone(), row.owner_group_id.clone())?;
    let (username, legacy) = resolve_creator(&state, row.created_by).await;
    let view = build_view(row, destination, username, legacy)?;
    Ok(Json(ThemeResponse { theme: view }))
}

/// `PATCH /api/themes/{id}` — rename and/or overwrite a theme's settings.
/// Personal owner or group owner only.
///
/// # Errors
///
/// Returns [`Error::Forbidden`] / [`Error::NotFound`] / [`Error::BadRequest`].
pub async fn update(
    user: CurrentUser,
    State(state): State<AppState>,
    Path(theme_id): Path<Uuid>,
    Json(body): Json<UpdateThemeRequest>,
) -> Result<Json<ThemeResponse>, Error> {
    if body.name.is_none() && body.settings.is_none() {
        return Err(Error::BadRequest(
            "PATCH must set at least one of `name` or `settings`".to_owned(),
        ));
    }
    let mut row = library::assert_can_modify_theme(&state.db, user.user_id, theme_id).await?;

    let new_name = match &body.name {
        Some(raw) => Some(library::sanitise_display_name(raw, "theme name")?),
        None => None,
    };
    let new_settings_json = match &body.settings {
        Some(settings) => {
            settings.validate()?;
            Some(serde_json::to_string(settings).map_err(|err| {
                tracing::error!("theme settings serialise failed: {err}");
                Error::Database
            })?)
        }
        None => None,
    };
    let now = Utc::now();
    sqlx::query(
        "UPDATE themes \
         SET name = COALESCE(?1, name), \
             settings_json = COALESCE(?2, settings_json), \
             updated_at = ?3 \
         WHERE theme_id = ?4",
    )
    .bind(new_name.as_deref())
    .bind(new_settings_json.as_deref())
    .bind(now)
    .bind(theme_id.as_bytes().to_vec())
    .execute(&state.db)
    .await
    .map_err(|err| {
        if error::is_unique_violation(&err) {
            return Error::BadRequest(
                "a theme with that name already exists in this scope; pick a different name"
                    .to_owned(),
            );
        }
        tracing::error!("update theme failed: {err}");
        Error::Database
    })?;

    // Reflect the applied changes back onto the row so the response matches
    // what just landed in the DB without a redundant SELECT.
    if let Some(name) = new_name {
        row.name = name;
    }
    if let Some(json) = new_settings_json {
        row.settings_json = json;
    }
    row.updated_at = now;
    let destination =
        library::destination_from_columns(row.owner_user_id.clone(), row.owner_group_id.clone())?;
    let (username, legacy) = resolve_creator(&state, row.created_by).await;
    let view = build_view(row, destination, username, legacy)?;
    Ok(Json(ThemeResponse { theme: view }))
}

/// `DELETE /api/themes/{id}` — delete a theme. Personal owner or group
/// owner only. Nothing references a theme, so the delete always succeeds.
///
/// # Errors
///
/// Returns [`Error::Forbidden`] / [`Error::NotFound`].
pub async fn delete(
    user: CurrentUser,
    State(state): State<AppState>,
    Path(theme_id): Path<Uuid>,
) -> Result<Response, Error> {
    library::assert_can_modify_theme(&state.db, user.user_id, theme_id).await?;
    sqlx::query("DELETE FROM themes WHERE theme_id = ?1")
        .bind(theme_id.as_bytes().to_vec())
        .execute(&state.db)
        .await
        .map_err(|err| {
            tracing::error!("delete theme failed: {err}");
            Error::Database
        })?;
    Ok((ReqwestStatusCode::NO_CONTENT, "").into_response())
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::expect_used,
        reason = "test code panics on failure for clearer output"
    )]

    use pretty_assertions::assert_eq;

    use super::{GlwStyleOverrides, ThemeSettings};

    /// A theme with every toggle off and no colours set — the baseline the
    /// individual cases tweak one field at a time.
    fn minimal() -> ThemeSettings {
        ThemeSettings {
            version: 1,
            missing_map_tile_enabled: false,
            missing_map_tile_color: None,
            missing_region_enabled: false,
            missing_region_color: None,
            draw_region_rectangles: false,
            draw_region_names: false,
            draw_region_coordinates: false,
            region_label_font_id: None,
            glw_style: GlwStyleOverrides::default(),
            glw_font_id: None,
            route_color: None,
        }
    }

    #[test]
    fn accepts_canonical_colors() {
        let mut s = minimal();
        s.missing_map_tile_color = Some("#ff0000".to_owned());
        s.route_color = Some("#00FF00".to_owned());
        s.glw_style.wind_color = Some("#123abc".to_owned());
        s.validate().expect("canonical colours validate");
    }

    #[test]
    fn rejects_malformed_top_level_color() {
        let mut s = minimal();
        s.missing_region_color = Some("ff0000".to_owned()); // missing '#'
        assert!(s.validate().is_err());
    }

    #[test]
    fn rejects_malformed_glw_color() {
        let mut s = minimal();
        s.glw_style.label_color = Some("#fff".to_owned()); // shorthand
        assert!(s.validate().is_err());
    }

    #[test]
    fn none_colors_are_allowed() {
        minimal()
            .validate()
            .expect("a theme with no colours validates");
    }

    #[test]
    fn round_trips_through_json() {
        let mut s = minimal();
        s.draw_region_names = true;
        s.region_label_font_id = Some("DejaVuSans.ttf".to_owned());
        s.route_color = Some("#abcdef".to_owned());
        let json = serde_json::to_string(&s).expect("serialise");
        let back: ThemeSettings = serde_json::from_str(&json).expect("deserialise");
        assert_eq!(back.draw_region_names, s.draw_region_names);
        assert_eq!(back.region_label_font_id, s.region_label_font_id);
        assert_eq!(back.route_color, s.route_color);
    }

    #[test]
    fn version_defaults_when_absent() {
        // A payload from an older client that predates the version field.
        let back: ThemeSettings = serde_json::from_str("{}").expect("deserialise empty");
        assert_eq!(back.version, 1);
        assert!(!back.missing_map_tile_enabled);
    }
}
