//! HTTP route handlers.

pub mod auth;
pub mod events;
pub mod index;
pub mod notecard;
pub mod pages;
pub mod render;
pub mod result;

use axum::Router;
use axum::middleware;
use axum::routing::{get, post};
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
/// * **Protected** — `/`, the notecard and render APIs. Wrapped in the
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
        .route("/static/style.css", get(index::style_css));

    let protected = Router::new()
        .route("/", get(index::index))
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
