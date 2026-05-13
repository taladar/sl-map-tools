//! HTTP route handlers.

pub mod events;
pub mod index;
pub mod notecard;
pub mod render;
pub mod result;

use axum::Router;
use axum::routing::{get, post};
use tower_http::compression::CompressionLayer;
use tower_http::trace::TraceLayer;

use crate::state::AppState;

/// Build the full axum [`Router`] wired to the given application state.
pub fn build(state: AppState) -> Router {
    Router::new()
        .route("/", get(index::index))
        .route("/static/app.js", get(index::app_js))
        .route("/static/style.css", get(index::style_css))
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
        .layer(CompressionLayer::new())
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
