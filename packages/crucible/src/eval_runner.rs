use aether::llm::{StreamingModelProvider, ToolDefinition};
use aether::mcp::run_mcp_task::McpCommand;
use aether::mcp::{mcp, ServerFactory};
use owo_colors::OwoColorize;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tracing::Instrument;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Layer};
use uuid::Uuid;

use crate::eval::{Eval, WorkingDirectory};
use crate::eval_assertion::{EvalAssertion, EvalAssertionResult};
use crate::eval_config::EvalsConfig;
use crate::server;
use crate::storage::{EvalResult, ResultsStore};

/// Configure and run AI agent evaluations with custom MCP servers
pub struct EvalRunner<T: ResultsStore> {
    output_dir: Option<PathBuf>,
    factories: HashMap<String, ServerFactory>,
    agent_prompt: Option<String>,
    mcp_json_path: Option<PathBuf>,
    results_store: T,
}

impl<T: ResultsStore + 'static> EvalRunner<T> {
    /// Create a new EvalRunner with the given results store
    pub fn new(results_store: T) -> Self {
        Self {
            output_dir: None,
            factories: HashMap::new(),
            agent_prompt: None,
            mcp_json_path: None,
            results_store,
        }
    }

    /// Set the output directory for logs and results
    pub fn with_output_dir(mut self, output_dir: PathBuf) -> Self {
        self.output_dir = Some(output_dir);
        self
    }

    /// Set the system prompt for the agent under eval
    pub fn with_agent_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.agent_prompt = Some(prompt.into());
        self
    }

    /// Set the path to mcp.json for agent under eval
    pub fn with_mcp_json(mut self, path: impl Into<PathBuf>) -> Self {
        self.mcp_json_path = Some(path.into());
        self
    }

    /// Register an InMemory MCP server factory
    ///
    /// # Arguments
    /// * `name` - The name of the server (referenced in mcp.json)
    /// * `factory` - Factory function that creates server instances
    pub fn with_mcp_server_factory(
        mut self,
        name: impl Into<String>,
        factory: ServerFactory,
    ) -> Self {
        self.factories.insert(name.into(), factory);
        self
    }

    /// Register multiple InMemory MCP server factories
    pub fn with_mcp_server_factories(mut self, factories: HashMap<String, ServerFactory>) -> Self {
        self.factories.extend(factories);
        self
    }

    /// Run the evaluations
    ///
    /// # Arguments
    /// * `evals` - Vector of evaluations to run
    /// * `config` - Configuration including LLM providers and batching settings
    ///
    /// # Returns
    /// Result containing the run ID
    pub async fn run_evals<M, J>(
        self,
        evals: Vec<Eval>,
        config: EvalsConfig<M, J>,
    ) -> Result<Uuid, Box<dyn std::error::Error>>
    where
        M: StreamingModelProvider + 'static,
        J: StreamingModelProvider + 'static,
    {
        if evals.is_empty() {
            return Err("No evals provided".into());
        }

        let run_id = Uuid::new_v4();
        println!(
            "\n{} {}",
            "Run ID:".bold(),
            run_id.to_string().bright_cyan()
        );

        let agent_prompt = self.agent_prompt;
        let results_store = Arc::new(self.results_store);
        let store_layer = results_store.create_tracing_layer(run_id);
        Self::setup_tracing(store_layer)?;

        let (server_handle, sse_tx) =
            Self::start_axum_server(&results_store, run_id, config.serve)?;
        let mcp_builder = Self::create_mcp_builder(self.factories, self.mcp_json_path)?;
        let (tool_definitions, mcp_tx, _mcp_handle) = mcp_builder.spawn().await?;

        // Wrap providers in Arc so they can be shared across tasks
        let llm = Arc::new(config.llm);
        let judge_llm = Arc::new(config.judge_llm);

        let batch_size = config.batch_size.unwrap_or(evals.len());
        let batch_delay = config.batch_delay.unwrap_or(Duration::ZERO);
        let total_evals = evals.len();

        // Process evals in batches
        let mut evals_iter = evals.into_iter();
        let mut batch_num = 0;
        loop {
            let batch: Vec<Eval> = evals_iter.by_ref().take(batch_size).collect();
            if batch.is_empty() {
                break;
            }
            batch_num += 1;

            tracing::info!(
                "Processing batch {}/{} ({} evals)",
                batch_num,
                total_evals.div_ceil(batch_size),
                batch.len()
            );

            Self::run_eval_batch(
                batch,
                run_id,
                &agent_prompt,
                &tool_definitions,
                &mcp_tx,
                &llm,
                &judge_llm,
                &sse_tx,
                &results_store,
            )
            .await;

            // Add delay between batches to prevent rate limiting
            if !batch_delay.is_zero() && batch_num * batch_size < total_evals {
                tracing::info!("Waiting {:?} before next batch...", batch_delay);
                tokio::time::sleep(batch_delay).await;
            }
        }

        // Notify that the run is complete
        if let Some(tx) = &sse_tx {
            let _ = tx.send(server::SseEvent::RunCompleted { run_id });
        }

        // Keep the server running if serving
        if config.serve {
            // Keep the server running (it's in a background task)
            println!(
                "\n{}",
                "Server is still running. Press Ctrl+C to exit."
                    .bold()
                    .green()
            );

            // Waits indefinitely until user hits Ctrl+C to exit
            tokio::signal::ctrl_c().await?;
            if let Some(handle) = server_handle {
                handle.abort();
            }
        }

        Ok(run_id)
    }

    // Private helper methods

    /// Set up tracing subscriber with store and fmt layers
    fn setup_tracing(
        store_layer: Box<dyn tracing_subscriber::Layer<tracing_subscriber::Registry> + Send + Sync>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let env_filter =
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

        let fmt_layer = tracing_subscriber::fmt::layer()
            .with_writer(std::io::stdout)
            .pretty();

        tracing_subscriber::registry()
            .with(store_layer)
            .with(fmt_layer.with_filter(env_filter))
            .try_init()
            .map_err(|e| -> Box<dyn std::error::Error> { e.into() })
    }

    fn create_mcp_builder(
        factories: HashMap<String, ServerFactory>,
        mcp_json_path: Option<PathBuf>,
    ) -> Result<aether::mcp::McpBuilder, Box<dyn std::error::Error>> {
        let mut mcp_builder = mcp();
        for (name, factory) in factories {
            mcp_builder = mcp_builder.register_in_memory_server(name, factory);
        }

        if let Some(mcp_json_path) = mcp_json_path {
            mcp_builder = mcp_builder.from_json_file(mcp_json_path.to_str().unwrap())?;
        }

        Ok(mcp_builder)
    }

    /// Capture git diffs (agent and reference) for GitRepo working directories
    fn capture_git_diffs(eval: &Eval, report: &mut EvalResult) {
        if let WorkingDirectory::GitRepo {
            path,
            start_commit,
            gold_commit,
            ..
        } = &eval.working_directory
        {
            use crate::storage::{DiffStats, GitDiff};
            let repo = crate::git_repo::GitRepo::from_path(path);

            // Capture agent diff (unstaged changes)
            if let Ok(agent_diff_str) = repo.diff_unstaged() {
                let stats = DiffStats::from_diff(&agent_diff_str);
                report.set_agent_diff(GitDiff {
                    diff: agent_diff_str,
                    stats,
                });
            }

            // Capture reference diff (gold/human solution)
            if let Ok(gold_diff_str) = repo.diff(start_commit, gold_commit) {
                let stats = DiffStats::from_diff(&gold_diff_str);
                report.set_reference_diff(GitDiff {
                    diff: gold_diff_str,
                    stats,
                });
            }
        }
    }

    /// Spawn a single eval task with tracing instrumentation
    fn spawn_eval_task<M, J>(
        eval: Eval,
        eval_id: Uuid,
        run_id: Uuid,
        agents_prompt: Option<String>,
        tool_definitions: Vec<ToolDefinition>,
        mcp_tx: mpsc::Sender<McpCommand>,
        llm: Arc<M>,
        judge_llm: Arc<J>,
        sse_tx: Option<tokio::sync::broadcast::Sender<server::SseEvent>>,
        results_store: Arc<T>,
    ) -> tokio::task::JoinHandle<(
        Eval,
        Uuid,
        Result<Vec<(EvalAssertion, EvalAssertionResult)>, Box<dyn std::error::Error + Send + Sync>>,
        Duration,
        Option<tokio::sync::broadcast::Sender<server::SseEvent>>,
    )>
    where
        M: StreamingModelProvider + 'static,
        J: StreamingModelProvider + 'static,
    {
        let eval_name = eval.name.clone();
        let eval_name_for_span = eval_name.clone();

        tokio::spawn(
            async move {
                // Save "started" state to store
                let started_result = EvalResult::started(&eval, eval_id);
                if let Err(e) = results_store.save_eval_result(run_id, &started_result).await {
                    tracing::warn!("Failed to write started state for {}: {}", eval.name, e);
                }

                // Broadcast eval started event
                if let Some(tx) = &sse_tx {
                    let _ = tx.send(server::SseEvent::EvalStarted {
                        run_id,
                        eval_id,
                        name: eval_name.clone(),
                    });
                }

                let start = Instant::now();
                let result = eval
                    .run(llm, judge_llm, tool_definitions, mcp_tx, agents_prompt)
                    .await;
                let duration = start.elapsed();
                (eval, eval_id, result, duration, sse_tx)
            }
            .instrument(tracing::info_span!("eval_task", eval_name = %eval_name_for_span, eval_id = %eval_id)),
        )
    }

    /// Handle the result of a single eval task
    async fn on_eval_result(
        task_result: Result<
            (
                Eval,
                Uuid,
                Result<
                    Vec<(EvalAssertion, EvalAssertionResult)>,
                    Box<dyn std::error::Error + Send + Sync>,
                >,
                Duration,
                Option<tokio::sync::broadcast::Sender<server::SseEvent>>,
            ),
            tokio::task::JoinError,
        >,
        results_store: &Arc<T>,
        run_id: Uuid,
    ) {
        match task_result {
            Ok((eval, eval_id, Ok(eval_results), _duration, sse_tx)) => {
                let mut report = EvalResult::completed(&eval, eval_id, &eval_results[..]);
                Self::capture_git_diffs(&eval, &mut report);

                if let Err(e) = results_store.save_eval_result(run_id, &report).await {
                    tracing::warn!("Failed to write result file for {}: {}", eval.name, e);
                }

                // Broadcast eval completed event
                if let Some(tx) = &sse_tx {
                    let _ = tx.send(server::SseEvent::EvalCompleted {
                        run_id,
                        eval_id,
                        name: report.eval_name().to_string(),
                        report: report.clone(),
                    });
                }
            }
            Ok((eval, _eval_id, Err(e), _duration, _sse_tx)) => {
                tracing::error!("Eval '{}' failed with error: {}", eval.name, e);
            }
            Err(e) => {
                tracing::error!("Task panicked: {}", e);
            }
        }
    }

    /// Start the axum server and return the task handle and SSE transmitter
    fn start_axum_server(
        results_store: &Arc<T>,
        run_id: Uuid,
        serve: bool,
    ) -> Result<
        (
            Option<JoinHandle<()>>,
            Option<tokio::sync::broadcast::Sender<server::SseEvent>>,
        ),
        Box<dyn std::error::Error>,
    > {
        if serve {
            let state = Arc::new(server::AppState::new(results_store.clone(), run_id));
            let sse_tx = Some(state.sse_tx.clone());
            let state_clone = state.as_ref().clone();
            let server_handle = Some(tokio::spawn(async move {
                if let Err(e) = server::serve(state_clone).await {
                    tracing::error!("Server error: {}", e);
                }
            }));
            Ok((server_handle, sse_tx))
        } else {
            Ok((None, None))
        }
    }

    /// Run a single batch of evaluations
    async fn run_eval_batch<M, J>(
        batch: Vec<Eval>,
        run_id: Uuid,
        agent_prompt: &Option<String>,
        tool_definitions: &Vec<ToolDefinition>,
        mcp_tx: &mpsc::Sender<McpCommand>,
        llm: &Arc<M>,
        judge_llm: &Arc<J>,
        sse_tx: &Option<tokio::sync::broadcast::Sender<server::SseEvent>>,
        results_store: &Arc<T>,
    ) where
        M: StreamingModelProvider + 'static,
        J: StreamingModelProvider + 'static,
    {
        let tasks: Vec<_> = batch
            .into_iter()
            .map(|eval| {
                let eval_id = Uuid::new_v4();
                Self::spawn_eval_task(
                    eval,
                    eval_id,
                    run_id,
                    agent_prompt.clone(),
                    tool_definitions.clone(),
                    mcp_tx.clone(),
                    llm.clone(),
                    judge_llm.clone(),
                    sse_tx.clone(),
                    results_store.clone(),
                )
            })
            .collect();

        // Await all tasks in this batch concurrently
        let results = futures::future::join_all(tasks).await;

        for result in results {
            Self::on_eval_result(result, results_store, run_id).await;
        }
    }
}
