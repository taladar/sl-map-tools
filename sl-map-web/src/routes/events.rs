//! Server-Sent Events stream for following a render job.

use std::convert::Infallible;
use std::sync::Arc;

use axum::extract::{Path, State};
use axum::response::Sse;
use axum::response::sse::{Event, KeepAlive};
use futures::Stream;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use uuid::Uuid;

use crate::error::Error;
use crate::jobs::{JobState, ProgressDto};
use crate::state::AppState;

/// `GET /api/render/{id}/events` — SSE stream of `ProgressDto`s for the
/// given job. The stream starts with any events already recorded (so a
/// late subscriber sees the full history) and ends with `done` (or
/// `error`) once the job finishes.
///
/// # Errors
///
/// Returns [`Error::JobNotFound`] if no job with the given id exists in
/// the job store.
pub async fn events(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, Error> {
    let Some(job) = state.jobs.get(id).await else {
        return Err(Error::JobNotFound);
    };
    let (tx, rx) = mpsc::channel::<Result<Event, Infallible>>(64);
    drop(tokio::spawn(pump_events(job, tx)));
    Ok(Sse::new(ReceiverStream::new(rx)).keep_alive(KeepAlive::default()))
}

/// Background task that drains the job's recorded events into an mpsc
/// channel until the job finishes or the SSE client disconnects.
async fn pump_events(job: Arc<JobState>, tx: mpsc::Sender<Result<Event, Infallible>>) {
    let mut cursor: usize = 0;
    let mut ping_rx = job.ping.subscribe();
    let mut outcome_rx = job.outcome.subscribe();
    loop {
        let snapshot: Vec<ProgressDto> = {
            let events = job.events.lock().await;
            events.get(cursor..).map(<[_]>::to_vec).unwrap_or_default()
        };
        if !snapshot.is_empty() {
            cursor = cursor.saturating_add(snapshot.len());
            for dto in snapshot {
                match serde_json::to_string(&dto) {
                    Ok(payload) => {
                        if tx.send(Ok(Event::default().data(payload))).await.is_err() {
                            return; // client disconnected
                        }
                    }
                    Err(err) => {
                        tracing::warn!("could not serialize progress event: {err}");
                    }
                }
            }
            continue;
        }
        if outcome_rx.borrow().is_some() {
            return;
        }
        tokio::select! {
            _ = ping_rx.recv() => {},
            _ = outcome_rx.changed() => {},
        }
    }
}
