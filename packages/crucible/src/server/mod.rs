use crate::report::{EvalReport, ReportData, SummaryReport, TraceEvent};
use axum::{
    Router,
    extract::State,
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
use std::{collections::HashMap, sync::RwLock};
use tokio::{net::TcpListener, sync::broadcast};
use tokio_stream::{Stream, StreamExt, wrappers::BroadcastStream};

/// Events sent over SSE to clients
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SseEvent {
    EvalStarted {
        name: String,
    },
    EvalCompleted {
        name: String,
        report: EvalReport,
    },
    TraceEvent {
        eval_name: String,
        trace: TraceEvent,
    },
    SummaryUpdated {
        summary: SummaryReport,
    },
}

/// Shared application state
#[derive(Clone)]
pub struct AppState {
    pub summary: Arc<RwLock<SummaryReport>>,
    pub eval_traces: Arc<RwLock<HashMap<String, Vec<TraceEvent>>>>,
    pub sse_tx: broadcast::Sender<SseEvent>,
}

impl AppState {
    pub fn new() -> Self {
        let (sse_tx, _rx) = broadcast::channel(100);
        Self {
            summary: Arc::new(RwLock::new(SummaryReport::new())),
            eval_traces: Arc::new(RwLock::new(HashMap::new())),
            sse_tx,
        }
    }

    /// Send an SSE event to all connected clients
    pub fn send_sse_event(&self, event: SseEvent) {
        let _ = self.sse_tx.send(event);
    }

    pub fn update_summary(&self, summary: SummaryReport) {
        *self.summary.write().unwrap() = summary.clone();
        self.send_sse_event(SseEvent::SummaryUpdated { summary });
    }

    pub fn add_traces(&self, eval_name: String, traces: Vec<TraceEvent>) {
        let mut guard = self.eval_traces.write().unwrap();
        guard.insert(eval_name, traces);
    }

    pub fn get_report_data(&self) -> ReportData {
        let summary = self.summary.read().unwrap().clone();
        let eval_traces = self.eval_traces.read().unwrap().clone();
        ReportData {
            summary,
            eval_traces,
        }
    }
}

pub async fn serve(state: AppState) -> Result<(), Box<dyn std::error::Error>> {
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

pub fn create_router(state: AppState) -> Router {
    Router::new()
        .route("/", get(serve_index))
        .route("/index.html", get(serve_index))
        .route("/styles.css", get(serve_styles))
        .route("/script.js", get(serve_script))
        .route("/api/report-data", get(get_report_data))
        .route("/api/events", get(sse_handler))
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

async fn get_report_data(State(state): State<AppState>) -> impl IntoResponse {
    let report_data = state.get_report_data();
    axum::Json(report_data)
}

async fn sse_handler(
    State(state): State<AppState>,
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
