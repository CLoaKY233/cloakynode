use std::sync::{Arc, atomic::Ordering};

use axum::{
    Json, Router,
    extract::{Query, State},
    response::{Html, IntoResponse},
    routing::get,
};
use serde::Deserialize;

use crate::{
    dashboard,
    models::HealthResponse,
    state::{AppState, SharedState},
};

#[derive(Debug, Deserialize)]
struct HistoryQuery {
    limit: Option<usize>,
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/api/current", get(current_sample))
        .route("/api/history", get(history))
        .route("/api/health", get(health))
        .with_state(state)
}

async fn index(State(state): State<AppState>) -> impl IntoResponse {
    bump_request_count(&state.shared);
    Html(dashboard::index_html(&state.shared.hostname))
}

async fn current_sample(State(state): State<AppState>) -> impl IntoResponse {
    bump_request_count(&state.shared);
    let current = {
        let inner = state.shared.inner.read().await;
        inner.current.clone()
    };
    Json(current)
}

async fn history(
    State(state): State<AppState>,
    Query(query): Query<HistoryQuery>,
) -> impl IntoResponse {
    bump_request_count(&state.shared);
    let limit = query.limit.unwrap_or(state.shared.history_size);
    let clamped = limit.clamp(1, state.shared.history_size);

    let history = {
        let inner = state.shared.inner.read().await;
        let len = inner.history.len();
        let start = len.saturating_sub(clamped);
        inner
            .history
            .iter()
            .skip(start)
            .cloned()
            .collect::<Vec<_>>()
    };

    Json(history)
}

async fn health(State(state): State<AppState>) -> impl IntoResponse {
    bump_request_count(&state.shared);
    let response = {
        let inner = state.shared.inner.read().await;
        HealthResponse {
            status: "ok",
            process_uptime_seconds: state.shared.started_at.elapsed().as_secs(),
            last_sample_timestamp_unix_ms: inner.current.timestamp_unix_ms,
            sample_errors: inner.sample_errors,
            request_count: state.shared.request_count.load(Ordering::Relaxed),
        }
    };
    Json(response)
}

fn bump_request_count(shared: &Arc<SharedState>) {
    shared.request_count.fetch_add(1, Ordering::Relaxed);
}

#[cfg(test)]
mod tests {
    use std::{collections::VecDeque, sync::atomic::AtomicU64, time::Instant};

    use axum::{body::Body, http::Request};
    use tower::util::ServiceExt;

    use super::*;
    use crate::{models::SystemSample, state::RuntimeState};

    #[tokio::test]
    async fn history_endpoint_clamps_limit() {
        let shared = Arc::new(SharedState {
            hostname: "pi".to_string(),
            started_at: Instant::now(),
            history_size: 3,
            request_count: AtomicU64::new(0),
            inner: tokio::sync::RwLock::new(RuntimeState {
                current: SystemSample {
                    timestamp_unix_ms: 3,
                    ..SystemSample::default()
                },
                history: VecDeque::from(vec![
                    SystemSample {
                        timestamp_unix_ms: 1,
                        ..SystemSample::default()
                    },
                    SystemSample {
                        timestamp_unix_ms: 2,
                        ..SystemSample::default()
                    },
                    SystemSample {
                        timestamp_unix_ms: 3,
                        ..SystemSample::default()
                    },
                ]),
                sample_errors: 0,
            }),
        });

        let app = router(AppState { shared });

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/history?limit=99")
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("response should be produced");

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("body should read");
        let samples: Vec<SystemSample> =
            serde_json::from_slice(&body).expect("history body should deserialize");

        assert_eq!(samples.len(), 3);
        assert_eq!(samples[0].timestamp_unix_ms, 1);
    }
}
