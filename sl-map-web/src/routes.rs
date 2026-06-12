//! HTTP route handlers.

pub mod auth;
pub mod events;
pub mod fonts;
pub mod glw;
pub mod groups;
pub mod index;
pub mod invitations;
pub mod library_pages;
pub mod logos;
pub mod notecard;
pub mod notecards;
pub mod pages;
pub mod render;
pub mod renders;
pub mod result;
pub mod text;
pub mod users;

use axum::Router;
use axum::extract::DefaultBodyLimit;
use axum::http::header;
use axum::middleware;
use axum::routing::{get, patch, post};
use tower_http::compression::CompressionLayer;
use tower_http::set_header::SetResponseHeaderLayer;
use tower_http::trace::TraceLayer;

/// Maximum HTTP request body size, in bytes. The largest legitimate upload
/// is a USB notecard, which never exceeds a few tens of KiB; 1 MiB leaves
/// generous headroom while preventing memory exhaustion from a malicious
/// multipart upload or JSON body.
const REQUEST_BODY_LIMIT: usize = 1024 * 1024;

/// Body-size limit for the logo upload route. Logo images are capped at
/// 5 MiB (enforced precisely in the handler); this adds headroom for the
/// multipart envelope and the `name` / `scope` fields. Applied as a
/// per-route override of [`REQUEST_BODY_LIMIT`].
const LOGO_REQUEST_BODY_LIMIT: usize = 6 * 1024 * 1024;

/// Content-Security-Policy applied to every response.
///
/// The pages load their JS via external `<script src="/static/...">` only —
/// no inline `<script>` blocks — so `script-src 'self'` is enough.
/// `style-src` allows `'unsafe-inline'` because the renderer page sets
/// element `.style` properties from JS for positional layout
/// (`element.style.width = ...`), which CSP treats as an inline style.
/// `img-src` also allows the Linden Lab map-tile CDN so the renderer
/// preview can display tiles, and `blob:` so the preview can display the
/// server-rendered GLW overlay (fetched as a blob and shown via
/// `URL.createObjectURL`).
const CONTENT_SECURITY_POLICY: &str = "default-src 'self'; \
     img-src 'self' https://secondlife-maps-cdn.akamaized.net blob:; \
     script-src 'self'; \
     style-src 'self' 'unsafe-inline'; \
     connect-src 'self'; \
     frame-ancestors 'none'; \
     base-uri 'none'; \
     form-action 'self'";

use crate::auth::require_session;
use crate::csrf::require_same_origin;
use crate::state::AppState;

/// Build the full axum [`Router`] wired to the given application state.
///
/// Routes are organised into three groups:
///
/// * **Public, open** — `/login`, `/set-password`, the static assets, the
///   LSL-facing `/api/auth/register`, and `/api/auth/set-password`. No
///   session and no same-origin requirement.
/// * **Public, CSRF-checked** — `/api/auth/login` and `/api/auth/logout`:
///   no session required, but wrapped in `require_same_origin` to block
///   login fixation and forced-logout from third-party origins.
/// * **Protected** — `/`, the notecard and render APIs, plus the new
///   groups/invitations/library endpoints and HTML pages. Wrapped in both
///   `require_same_origin` and `require_session`.
/// * All three groups are merged and then wrapped in compression +
///   tracing.
pub fn build(state: AppState) -> Router {
    // Public endpoints that are CSRF-checked. `/api/auth/register` is
    // excluded because it is called from `llHTTPRequest`, which does not
    // send an `Origin` header — its only authenticator is the pre-shared
    // bearer token. `/api/auth/set-password` is excluded because the only
    // capability it grants is bound to a one-time secret token that the
    // attacker cannot guess cross-site.
    let public_csrf = Router::new()
        .route("/api/auth/login", post(auth::login))
        .route("/api/auth/logout", post(auth::logout))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            require_same_origin,
        ));

    let public_open = Router::new()
        .route("/login", get(pages::login_page))
        .route("/set-password", get(pages::set_password_page))
        .route("/api/auth/register", post(auth::register))
        .route("/api/auth/set-password", post(auth::set_password))
        .route("/api/auth/me", get(auth::me))
        .route("/static/app.js", get(index::app_js))
        .route("/static/style.css", get(index::style_css))
        .route("/static/library.js", get(library_pages::library_js))
        .route("/static/groups.js", get(library_pages::groups_js))
        .route("/static/invitations.js", get(library_pages::invitations_js))
        .route("/static/profile.js", get(library_pages::profile_js))
        .route("/static/modal.js", get(library_pages::modal_js))
        .route("/static/login.js", get(pages::login_js))
        .route("/static/set_password.js", get(pages::set_password_js));

    let protected = Router::new()
        .route("/", get(index::index))
        .route("/library", get(library_pages::library))
        .route("/groups", get(library_pages::groups))
        .route("/groups/{id}", get(library_pages::groups))
        .route("/invitations", get(library_pages::invitations))
        .route(
            "/api/notecard/derive-rectangle",
            post(notecard::derive_rectangle),
        )
        .route("/api/render/grid-rectangle", post(render::grid_rectangle))
        .route("/api/render/usb-notecard", post(render::usb_notecard))
        // read-only: where on the map (corners / side midpoints / centre) is
        // free of overlay content, so the UI can offer placement of a legend,
        // logo or label before the final render
        .route(
            "/api/render/placement-slots/grid-rectangle",
            post(render::free_placement_slots_grid_rectangle),
        )
        .route(
            "/api/render/placement-slots/usb-notecard",
            post(render::free_placement_slots_usb_notecard),
        )
        // read-only: rasterise just the GLW overlay as a transparent PNG sized
        // to the final-image bounds, for the client-side preview to composite
        // over the map tiles
        .route("/api/render/glw-preview", post(render::glw_preview))
        // read-only: rasterise just the GLW base legend as a transparent PNG at
        // the final-image resolution, for the preview to composite into the
        // bounds rectangle at the legend's chosen slot
        .route(
            "/api/render/glw-legend-preview",
            post(render::glw_legend_preview),
        )
        // read-only: rasterise the text labels + logos as a transparent PNG at
        // the final-image resolution, for the preview to composite into the
        // bounds rectangle at each placement's chosen slot
        .route(
            "/api/render/placement-preview/grid-rectangle",
            post(render::placement_preview_grid_rectangle),
        )
        .route(
            "/api/render/placement-preview/usb-notecard",
            post(render::placement_preview_usb_notecard),
        )
        .route("/api/render/{id}/events", get(events::events))
        .route("/api/render/{id}/image", get(result::image))
        .route(
            "/api/render/{id}/image-without-route",
            get(result::image_without_route),
        )
        .route("/api/render/{id}/metadata", get(result::metadata))
        // available fonts (for the GLW label dropdown)
        .route("/api/fonts", get(fonts::list))
        // rendered text size for the label placement-fit check
        .route("/api/text/measure", post(text::measure))
        // groups
        .route("/api/groups", get(groups::list_mine).post(groups::create))
        .route(
            "/api/groups/{id}",
            get(groups::get)
                .patch(groups::rename)
                .delete(groups::delete),
        )
        .route("/api/groups/{id}/members", get(groups::list_members))
        .route(
            "/api/groups/{id}/members/{user_id}",
            patch(groups::set_member_role).delete(groups::remove_member),
        )
        .route("/api/groups/{id}/leave", post(groups::leave))
        // invitations
        .route(
            "/api/groups/{id}/invitations",
            get(invitations::list_for_group).post(invitations::create),
        )
        .route("/api/invitations", get(invitations::list_mine))
        .route("/api/invitations/{id}/accept", post(invitations::accept))
        .route("/api/invitations/{id}/reject", post(invitations::reject))
        // saved notecards
        .route(
            "/api/notecards",
            get(notecards::list).post(notecards::create),
        )
        .route(
            "/api/notecards/{id}",
            get(notecards::get).delete(notecards::delete),
        )
        .route("/api/notecards/{id}/text", get(notecards::download_text))
        // saved renders
        .route("/api/renders", get(renders::list))
        .route(
            "/api/renders/{id}",
            get(renders::get).delete(renders::delete),
        )
        .route("/api/renders/{id}/image", get(renders::image))
        .route(
            "/api/renders/{id}/image-without-route",
            get(renders::image_without_route),
        )
        .route("/api/renders/{id}/download", get(renders::download))
        .route(
            "/api/renders/{id}/download-without-route",
            get(renders::download_without_route),
        )
        .route("/api/renders/{id}/metadata", get(renders::metadata))
        .route("/api/renders/{id}/settings", get(renders::settings))
        // saved GLW data
        .route("/api/glw", get(glw::list))
        // default style-override values for the render form's GLW
        // colour swatches (static segment, registered before `{id}`)
        .route("/api/glw/style-defaults", get(glw::style_defaults))
        .route(
            "/api/glw/{id}",
            get(glw::get).patch(glw::rename).delete(glw::delete),
        )
        .route("/api/glw/{id}/payload", get(glw::payload))
        // saved logos. The upload route raises the body limit above the
        // global default so a 5 MiB image fits; the GET shares the limit
        // harmlessly.
        .route(
            "/api/logos",
            get(logos::list)
                .post(logos::create)
                .layer(DefaultBodyLimit::max(LOGO_REQUEST_BODY_LIMIT)),
        )
        .route(
            "/api/logos/{id}",
            get(logos::get).patch(logos::rename).delete(logos::delete),
        )
        .route("/api/logos/{id}/image", get(logos::image))
        // profile + account
        .route("/profile", get(library_pages::profile))
        .route("/profile/{id}", get(library_pages::profile))
        .route("/api/users/me", axum::routing::delete(users::delete_me))
        .route(
            "/api/users/me/preferences",
            patch(users::update_preferences),
        )
        .route("/api/users/{id}", get(users::get))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            require_session,
        ))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            require_same_origin,
        ));

    public_open
        .merge(public_csrf)
        .merge(protected)
        .layer(DefaultBodyLimit::max(REQUEST_BODY_LIMIT))
        // Response security headers. `if_not_present` so per-route
        // handlers can override (none currently do).
        .layer(SetResponseHeaderLayer::if_not_present(
            header::CONTENT_SECURITY_POLICY,
            axum::http::HeaderValue::from_static(CONTENT_SECURITY_POLICY),
        ))
        .layer(SetResponseHeaderLayer::if_not_present(
            header::X_CONTENT_TYPE_OPTIONS,
            axum::http::HeaderValue::from_static("nosniff"),
        ))
        .layer(SetResponseHeaderLayer::if_not_present(
            header::REFERRER_POLICY,
            axum::http::HeaderValue::from_static("strict-origin-when-cross-origin"),
        ))
        .layer(SetResponseHeaderLayer::if_not_present(
            header::X_FRAME_OPTIONS,
            axum::http::HeaderValue::from_static("DENY"),
        ))
        // Gzip every response. Safe so long as no response body contains a
        // per-user secret — BREACH (Browser Reconnaissance and Exfiltration
        // via Adaptive Compression of Hypertext) lets an attacker who can
        // reflect input next to a secret extract the secret byte-by-byte by
        // observing compressed response sizes.
        //
        // Today the inventory is clean:
        //   - Session id lives only in a signed cookie, never in a body.
        //   - There is no body-embedded CSRF token; CSRF is enforced via
        //     `csrf::require_same_origin` on the Origin header.
        //   - The set-password token is returned only by `/api/auth/register`,
        //     which is bearer-authenticated and only callable from the
        //     in-world LSL script — a browser-driven attacker cannot induce
        //     the victim to issue that request.
        //
        // A future change that puts ANY of the following in a response body
        // breaks the contract and requires either splitting the compression
        // layer to exclude the affected endpoints, or masking the secret
        // per-response with a random XOR-pad:
        //   - A CSRF token (e.g. switching to double-submit-cookie or a
        //     hidden form token rendered into HTML).
        //   - An OAuth state value, magic-link token, integration API key,
        //     or any other long-lived per-user secret.
        //   - Reflecting attacker-influenced input adjacent to existing
        //     identifiers in a way that lets size oracles narrow them down.
        .layer(CompressionLayer::new())
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::CONTENT_SECURITY_POLICY;

    /// The preview displays the server-rendered GLW overlay via a `blob:` URL
    /// (`URL.createObjectURL`), so the `img-src` directive must permit `blob:`
    /// or the browser blocks the overlay image.
    #[test]
    fn csp_img_src_allows_blob() {
        let img_src_allows_blob = CONTENT_SECURITY_POLICY
            .split(';')
            .map(str::trim)
            .filter(|d| d.starts_with("img-src"))
            .any(|d| d.split_whitespace().any(|s| s == "blob:"));
        assert!(
            img_src_allows_blob,
            "img-src must allow blob: for the GLW preview overlay; CSP was: {CONTENT_SECURITY_POLICY}"
        );
    }
}
