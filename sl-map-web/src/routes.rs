//! HTTP route handlers.

pub mod auth;
pub mod events;
pub mod groups;
pub mod index;
pub mod invitations;
pub mod library_pages;
pub mod notecard;
pub mod notecards;
pub mod pages;
pub mod render;
pub mod renders;
pub mod result;

use axum::Router;
use axum::middleware;
use axum::routing::{get, patch, post};
use tower_http::compression::CompressionLayer;
use tower_http::trace::TraceLayer;

use crate::auth::require_session;
use crate::state::AppState;

/// Build the full axum [`Router`] wired to the given application state.
///
/// Routes are organised into three groups:
///
/// * **Public** — `/login`, `/set-password`, the static assets, and the
///   `/api/auth/*` endpoints. No session required.
/// * **Protected** — `/`, the notecard and render APIs, plus the new
///   groups/invitations/library endpoints and HTML pages. Wrapped in the
///   `require_session` middleware.
/// * The two groups are merged and then wrapped in compression + tracing.
pub fn build(state: AppState) -> Router {
    let public = Router::new()
        .route("/login", get(pages::login_page))
        .route("/set-password", get(pages::set_password_page))
        .route("/api/auth/register", post(auth::register))
        .route("/api/auth/set-password", post(auth::set_password))
        .route("/api/auth/login", post(auth::login))
        .route("/api/auth/logout", post(auth::logout))
        .route("/api/auth/me", get(auth::me))
        .route("/static/app.js", get(index::app_js))
        .route("/static/style.css", get(index::style_css))
        .route("/static/library.js", get(library_pages::library_js))
        .route("/static/groups.js", get(library_pages::groups_js))
        .route("/static/invitations.js", get(library_pages::invitations_js));

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
        .route("/api/render/{id}/events", get(events::events))
        .route("/api/render/{id}/image", get(result::image))
        .route(
            "/api/render/{id}/image-without-route",
            get(result::image_without_route),
        )
        .route("/api/render/{id}/metadata", get(result::metadata))
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
        .route("/api/renders/{id}/metadata", get(renders::metadata))
        .route("/api/renders/{id}/settings", get(renders::settings))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            require_session,
        ));

    public
        .merge(protected)
        .layer(CompressionLayer::new())
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
