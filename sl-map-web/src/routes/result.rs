//! Handlers that serve the finished artifacts of a render job.

use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, State};
use axum::http::header;
use axum::response::{IntoResponse as _, Response};
use bytes::Bytes;
use uuid::Uuid;

use crate::auth::CurrentUser;
use crate::error::Error;
use crate::jobs::{JobOutcome, JobState, Metadata};
use crate::library;
use crate::state::AppState;

/// `GET /api/render/{id}/image` — serve the primary rendered image.
///
/// # Errors
///
/// Returns [`Error::NotFound`] if the render does not exist or is not
/// visible to the caller, [`Error::JobNotFound`] if the in-memory job has
/// been evicted, [`Error::JobNotFinished`] if the render is still in
/// progress, or [`Error::RenderFailed`] if the render task ended with an
/// error.
pub async fn image(
    user: CurrentUser,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Response, Error> {
    library::assert_can_read_render(&state.db, user.user_id, id).await?;
    let job = job(&state, id).await?;
    let outcome = await_outcome(&job)?;
    let JobOutcome::Ok {
        image,
        content_type,
        ..
    } = outcome.as_ref()
    else {
        return Err(render_error(&outcome));
    };
    Ok(image_response(image.clone(), content_type))
}

/// `GET /api/render/{id}/image-without-route` — serve the without-route
/// image if one was requested with the render.
///
/// # Errors
///
/// As for [`image()`], plus [`Error::NotFound`] if the render did not save a
/// without-route image.
pub async fn image_without_route(
    user: CurrentUser,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Response, Error> {
    library::assert_can_read_render(&state.db, user.user_id, id).await?;
    let job = job(&state, id).await?;
    let outcome = await_outcome(&job)?;
    let JobOutcome::Ok {
        image_without_route,
        content_type,
        ..
    } = outcome.as_ref()
    else {
        return Err(render_error(&outcome));
    };
    let Some(image) = image_without_route.as_ref() else {
        return Err(Error::NotFound(
            "this job did not save a without-route image".to_owned(),
        ));
    };
    Ok(image_response(image.clone(), content_type))
}

/// `GET /api/render/{id}/metadata` — serve the metadata JSON.
///
/// # Errors
///
/// As for [`image()`].
pub async fn metadata(
    user: CurrentUser,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Metadata>, Error> {
    library::assert_can_read_render(&state.db, user.user_id, id).await?;
    let job = job(&state, id).await?;
    let outcome = await_outcome(&job)?;
    let JobOutcome::Ok { metadata, .. } = outcome.as_ref() else {
        return Err(render_error(&outcome));
    };
    Ok(Json(metadata.clone()))
}

/// Look up a job or return `JobNotFound`.
async fn job(state: &AppState, id: Uuid) -> Result<Arc<JobState>, Error> {
    state.jobs.get(id).await.ok_or(Error::JobNotFound)
}

/// Return the current outcome or `JobNotFinished` if still running.
fn await_outcome(job: &Arc<JobState>) -> Result<Arc<JobOutcome>, Error> {
    let borrowed = job.outcome.borrow();
    borrowed
        .as_ref()
        .map_or(Err(Error::JobNotFinished), |o| Ok(Arc::clone(o)))
}

/// Translate an `Err` outcome to a render-failed error.
fn render_error(outcome: &Arc<JobOutcome>) -> Error {
    match outcome.as_ref() {
        JobOutcome::Err(msg) => Error::RenderFailed(msg.clone()),
        JobOutcome::Ok { .. } => Error::RenderFailed(
            "expected error outcome but found Ok (internal logic error)".to_owned(),
        ),
    }
}

/// Build an HTTP response for image bytes.
fn image_response(image: Bytes, content_type: &'static str) -> Response {
    ([(header::CONTENT_TYPE, content_type)], image).into_response()
}
