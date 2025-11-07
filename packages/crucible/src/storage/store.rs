use std::future::Future;
use tracing_subscriber::Layer;
use tracing_subscriber::Registry;
use uuid::Uuid;

use crate::storage::{EvalResult, TraceEvent};

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

/// Trait for storing and retrieving evaluation results, traces, and summaries
pub trait ResultsStore: Send + Sync + Clone {
    /// List all run IDs that have been stored
    fn get_run_ids(&self) -> impl Future<Output = Result<Vec<Uuid>>> + Send;

    /// Write an individual eval result for a specific run
    fn save_eval_result(
        &self,
        run_id: Uuid,
        report: &EvalResult,
    ) -> impl Future<Output = Result<()>> + Send;

    /// Read all eval results for a given run
    fn get_eval_results(
        &self,
        run_id: Uuid,
    ) -> impl Future<Output = Result<Vec<EvalResult>>> + Send;

    /// Get a specific eval result by its ID
    fn get_eval_result(
        &self,
        run_id: Uuid,
        eval_id: Uuid,
    ) -> impl Future<Output = Result<Option<EvalResult>>> + Send;

    /// Get traces/spans for a specific eval within a run
    fn get_eval_traces(
        &self,
        run_id: Uuid,
        eval_id: Uuid,
    ) -> impl Future<Output = Result<Vec<TraceEvent>>> + Send;

    /// Create a tracing layer that writes to this store for a specific run
    fn create_tracing_layer(&self, run_id: Uuid) -> Box<dyn Layer<Registry> + Send + Sync>;
}
