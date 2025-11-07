use crate::storage::{EvalResult, ResultsStore, TraceEvent};
use axum::{
    Router,
    extract::{Path, State},
    http::{StatusCode, header},
    response::{
        Html, IntoResponse, Response,
        sse::{Event, KeepAlive, Sse},
    },
    routing::get,
};
use owo_colors::OwoColorize;
use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use std::sync::Arc;
use std::sync::RwLock;
use tokio::{net::TcpListener, sync::broadcast};
use tokio_stream::{Stream, StreamExt, wrappers::BroadcastStream};
use uuid::Uuid;

/// Events sent over SSE to clients
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SseEvent {
    EvalStarted {
        run_id: Uuid,
        eval_id: Uuid,
        name: String,
    },
    EvalCompleted {
        run_id: Uuid,
        eval_id: Uuid,
        name: String,
        report: EvalResult,
    },
    TraceEvent {
        run_id: Uuid,
        eval_id: Uuid,
        eval_name: String,
        trace: TraceEvent,
    },
    RunCompleted {
        run_id: Uuid,
    },
}

impl SseEvent {
    /// Get the run_id associated with this event
    pub fn run_id(&self) -> Uuid {
        match self {
            SseEvent::EvalStarted { run_id, .. } => *run_id,
            SseEvent::EvalCompleted { run_id, .. } => *run_id,
            SseEvent::TraceEvent { run_id, .. } => *run_id,
            SseEvent::RunCompleted { run_id } => *run_id,
        }
    }

    /// Get the eval_id associated with this event, if applicable
    pub fn eval_id(&self) -> Option<Uuid> {
        match self {
            SseEvent::EvalStarted { eval_id, .. } => Some(*eval_id),
            SseEvent::EvalCompleted { eval_id, .. } => Some(*eval_id),
            SseEvent::TraceEvent { eval_id, .. } => Some(*eval_id),
            SseEvent::RunCompleted { .. } => None,
        }
    }
}

/// Shared application state
pub struct AppState<T: ResultsStore> {
    pub sse_tx: broadcast::Sender<SseEvent>,
    pub results_store: Arc<T>,
    pub current_run_id: Arc<RwLock<Option<Uuid>>>,
}

impl<T: ResultsStore> Clone for AppState<T> {
    fn clone(&self) -> Self {
        Self {
            sse_tx: self.sse_tx.clone(),
            results_store: Arc::clone(&self.results_store),
            current_run_id: Arc::clone(&self.current_run_id),
        }
    }
}

impl<T: ResultsStore> AppState<T> {
    pub fn new(results_store: Arc<T>, run_id: Uuid) -> Self {
        let (sse_tx, _rx) = broadcast::channel(100);
        Self {
            sse_tx,
            results_store,
            current_run_id: Arc::new(RwLock::new(Some(run_id))),
        }
    }

    /// Send an SSE event to all connected clients
    pub fn send_sse_event(&self, event: SseEvent) {
        let _ = self.sse_tx.send(event);
    }
}

pub async fn serve<T: ResultsStore + Clone + 'static>(
    state: AppState<T>,
) -> Result<(), Box<dyn std::error::Error>> {
    let app = create_router(state);
    let listener = TcpListener::bind("127.0.0.1:3000").await?;

    println!("\n{}", "=== Eval Report Server ===".bold().green());
    println!(
        "Report available at: {}",
        "http://localhost:3000".bold().cyan()
    );
    println!("Press {} to stop the server\n", "Ctrl+C".bold());

    axum::serve(listener, app).await?;
    Ok(())
}

pub fn create_router<T: ResultsStore + Clone + 'static>(state: AppState<T>) -> Router {
    Router::new()
        .route("/", get(serve_index))
        .route("/index.html", get(serve_index))
        .route("/styles.css", get(serve_styles))
        .route("/script.js", get(serve_script))
        .route("/api/events", get(sse_handler::<T>))
        .route("/api/runs", get(list_runs::<T>))
        .route(
            "/api/runs/:run_id",
            get(|state, path| get_run::<T>(state, path)),
        )
        .route(
            "/api/runs/:run_id/events",
            get(|state, path| run_sse_handler::<T>(state, path)),
        )
        .route(
            "/api/runs/:run_id/evals/:eval_id",
            get(|state, path| get_run_eval::<T>(state, path)),
        )
        .route(
            "/api/runs/:run_id/evals/:eval_id/events",
            get(|state, path| eval_sse_handler::<T>(state, path)),
        )
        .route(
            "/api/runs/:run_id/evals/:eval_id/traces",
            get(|state, path| get_eval_traces_handler::<T>(state, path)),
        )
        .with_state(state)
}

async fn serve_index() -> Html<&'static str> {
    Html(include_str!("./templates/index.html"))
}

async fn serve_styles() -> impl IntoResponse {
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/css; charset=utf-8")
        .body(include_str!("./templates/styles.css").to_string())
        .unwrap()
}

async fn serve_script() -> impl IntoResponse {
    Response::builder()
        .status(StatusCode::OK)
        .header(
            header::CONTENT_TYPE,
            "application/javascript; charset=utf-8",
        )
        .body(include_str!("./templates/script.js").to_string())
        .unwrap()
}

/// Global SSE handler - sends all events (deprecated, use scoped endpoints)
async fn sse_handler<T: ResultsStore>(
    State(state): State<AppState<T>>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let rx = state.sse_tx.subscribe();
    let stream = BroadcastStream::new(rx);
    let event_stream = stream.filter_map(|result| {
        result
            .ok()
            .and_then(|event| {
                serde_json::to_string(&event)
                    .map(|json| Event::default().data(json))
                    .map_err(|e| tracing::error!("Failed to serialize SSE event: {}", e))
                    .ok()
            })
            .map(Ok)
    });

    Sse::new(event_stream).keep_alive(KeepAlive::default())
}

/// Run-scoped SSE handler - only sends events for a specific run
async fn run_sse_handler<T: ResultsStore>(
    State(state): State<AppState<T>>,
    Path(run_id): Path<Uuid>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let rx = state.sse_tx.subscribe();
    let stream = BroadcastStream::new(rx);
    let event_stream = stream.filter_map(move |result| {
        result
            .ok()
            .filter(|event| event.run_id() == run_id)
            .and_then(|event| {
                serde_json::to_string(&event)
                    .map(|json| Event::default().data(json))
                    .map_err(|e| tracing::error!("Failed to serialize SSE event: {}", e))
                    .ok()
            })
            .map(Ok)
    });

    Sse::new(event_stream).keep_alive(KeepAlive::default())
}

/// Eval-scoped SSE handler - only sends events for a specific eval within a run
async fn eval_sse_handler<T: ResultsStore>(
    State(state): State<AppState<T>>,
    Path((run_id, eval_id)): Path<(Uuid, Uuid)>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let rx = state.sse_tx.subscribe();
    let stream = BroadcastStream::new(rx);
    let event_stream = stream.filter_map(move |result| {
        result
            .ok()
            .filter(|event| {
                event.run_id() == run_id && event.eval_id().map_or(false, |eid| eid == eval_id)
            })
            .and_then(|event| {
                serde_json::to_string(&event)
                    .map(|json| Event::default().data(json))
                    .map_err(|e| tracing::error!("Failed to serialize SSE event: {}", e))
                    .ok()
            })
            .map(Ok)
    });

    Sse::new(event_stream).keep_alive(KeepAlive::default())
}

/// List all runs
async fn list_runs<T: ResultsStore>(State(state): State<AppState<T>>) -> impl IntoResponse {
    match state.results_store.get_all_run_ids().await {
        Ok(runs) => axum::Json(runs).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to list runs: {}", e),
        )
            .into_response(),
    }
}

/// Get a specific run's eval results
async fn get_run<T: ResultsStore>(
    State(state): State<AppState<T>>,
    Path(run_id): Path<Uuid>,
) -> impl IntoResponse {
    match state.results_store.get_eval_results(run_id).await {
        Ok(results) => axum::Json(results).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to read run: {}", e),
        )
            .into_response(),
    }
}

/// Get a specific eval result from a specific run
async fn get_run_eval<T: ResultsStore>(
    State(state): State<AppState<T>>,
    Path((run_id, eval_id)): Path<(Uuid, Uuid)>,
) -> impl IntoResponse {
    match state.results_store.get_eval_result(run_id, eval_id).await {
        Ok(Some(result)) => axum::Json(result).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            format!("Eval {} not found in run {}", eval_id, run_id),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to read eval result: {}", e),
        )
            .into_response(),
    }
}

/// Get traces for a specific eval within a run
async fn get_eval_traces_handler<T: ResultsStore>(
    State(state): State<AppState<T>>,
    Path((run_id, eval_id)): Path<(Uuid, Uuid)>,
) -> impl IntoResponse {
    match state.results_store.get_eval_traces(run_id, eval_id).await {
        Ok(traces) => axum::Json(traces).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to read traces: {}", e),
        )
            .into_response(),
    }
}
