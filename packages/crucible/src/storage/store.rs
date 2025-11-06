use std::collections::HashMap;
use std::future::Future;
use tracing_subscriber::Layer;
use tracing_subscriber::Registry;
use uuid::Uuid;

use crate::storage::EvalResult;
use crate::storage::RunResult;
use crate::storage::TraceEvent;

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

/// Trait for storing and retrieving evaluation results, traces, and summaries
pub trait ResultsStore: Send + Sync + Clone {
    /// Write an individual eval result for a specific run
    fn save_eval_result(
        &self,
        run_id: Uuid,
        eval_name: &str,
        report: &EvalResult,
    ) -> impl Future<Output = Result<()>> + Send;

    /// Write the summary report for a specific run
    fn save_run_result(
        &self,
        run_id: Uuid,
        result: &RunResult,
    ) -> impl Future<Output = Result<()>> + Send;

    /// Read all traces grouped by eval name for a specific run
    fn save_trace_events(
        &self,
        run_id: Uuid,
    ) -> impl Future<Output = Result<HashMap<String, Vec<TraceEvent>>>> + Send;

    /// Read the summary report for a specific run
    fn get_run_result(&self, run_id: Uuid) -> impl Future<Output = Result<RunResult>> + Send;

    /// Read an individual eval result for a specific run
    fn get_eval_result(
        &self,
        run_id: Uuid,
        eval_name: &str,
    ) -> impl Future<Output = Result<EvalResult>> + Send;

    /// Create a tracing layer that writes to this store for a specific run
    fn create_tracing_layer(&self, run_id: Uuid) -> Box<dyn Layer<Registry> + Send + Sync>;
}
