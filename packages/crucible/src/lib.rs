mod assertions;
pub mod eval;
pub mod eval_assertion;
pub mod eval_messages;
pub mod git_repo;
pub mod hooks;
pub mod metrics;
pub mod server;
pub mod storage;

pub use eval::{Eval, WorkingDirectory};
pub use eval_assertion::{EvalAssertion, EvalAssertionResult, LlmJudgeContext, ToolCallCount};
pub use eval_messages::EvalMessage;
pub use metrics::{BinaryMetric, EvalMetric, NumericMetric};
pub use server::{AppState, SseEvent};
pub use storage::{FileSystemStore, Result as StoreResult, ResultsStore};

use aether::llm::StreamingModelProvider;
use aether::mcp::{ServerFactory, mcp};
use owo_colors::OwoColorize;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::Instrument;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Layer};
use uuid::Uuid;

use crate::storage::{EvalReport, EvalResult};

pub struct EvalsConfig<T, U> {
    llm: T,
    judge_llm: U,
    batch_size: Option<usize>,
    batch_delay: Option<Duration>,
    serve: bool,
}

impl<T, U> EvalsConfig<T, U> {
    pub fn new(llm: T, judge_llm: U) -> Self {
        Self {
            llm,
            judge_llm,
            batch_size: None,
            batch_delay: None,
            serve: false,
        }
    }

    /// Set the batch size for concurrent evaluation execution
    ///
    /// When running many evaluations, you may want to limit the number of concurrent
    /// evaluations to avoid rate limiting with LLM providers. This setting controls
    /// how many evals are run concurrently in each batch.
    ///
    /// # Arguments
    /// * `batch_size` - Number of evals to run concurrently in each batch
    ///
    /// # Example
    /// ```no_run
    /// use crucible::EvalsConfig;
    /// use std::time::Duration;
    /// // let llm = ...; // Your LLM provider
    /// // let judge_llm = ...; // Your judge LLM provider
    /// // let config = EvalsConfig::new(llm, judge_llm)
    /// //     .with_batch_size(3)  // Run 3 evals at a time
    /// //     .with_batch_delay(Duration::from_secs(2));  // Wait 2 seconds between batches
    /// ```
    pub fn with_batch_size(mut self, batch_size: usize) -> Self {
        self.batch_size = Some(batch_size);
        self
    }

    /// Set the delay between batches
    ///
    /// This adds a delay between processing batches to further prevent rate limiting.
    /// The delay is only applied between batches, not within a batch.
    ///
    /// # Arguments
    /// * `delay` - Delay between batches to prevent rate limiting
    ///
    /// # Example
    /// ```
    /// use std::time::Duration;
    /// let delay = Duration::from_millis(2000);
    /// ```
    pub fn with_batch_delay(mut self, delay: Duration) -> Self {
        self.batch_delay = Some(delay);
        self
    }

    /// Enable serving the HTML report on localhost:3000 after evals complete
    ///
    /// When enabled, after all evaluations finish, a web server will start on
    /// localhost:3000 to serve the interactive HTML report. The server will run
    /// until interrupted with Ctrl+C.
    ///
    /// # Example
    /// ```no_run
    /// use crucible::EvalsConfig;
    /// // let llm = ...; // Your LLM provider
    /// // let judge_llm = ...; // Your judge LLM provider
    /// // let config = EvalsConfig::new(llm, judge_llm)
    /// //     .with_serve(true);  // Start web server after evals
    /// ```
    pub fn with_serve(mut self, serve: bool) -> Self {
        self.serve = serve;
        self
    }
}

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
    /// Result containing the summary report
    pub async fn run_evals<M, J>(
        self,
        evals: Vec<Eval>,
        config: EvalsConfig<M, J>,
    ) -> Result<EvalReport, Box<dyn std::error::Error>>
    where
        M: StreamingModelProvider + 'static,
        J: StreamingModelProvider + 'static,
    {
        if evals.is_empty() {
            return Err("No evals provided".into());
        }

        // Generate a unique run ID for this evaluation run
        let run_id = Uuid::new_v4();

        println!(
            "\n{} {}",
            "Run ID:".bold(),
            run_id.to_string().bright_cyan()
        );

        // Extract fields from self before using helper methods
        let agent_prompt = self.agent_prompt;
        let results_store = Arc::new(self.results_store);

        // Setup tracing subscriber
        Self::setup_tracing_helper(run_id, &results_store)?;

        // Create app state for SSE if serving
        let app_state = if config.serve {
            Some(Arc::new(server::AppState::new(
                results_store.clone(),
                run_id,
            )))
        } else {
            None
        };

        let server_handle = if config.serve {
            if let Some(state) = app_state.as_ref() {
                let state_clone = state.as_ref().clone();
                Some(tokio::spawn(async move {
                    if let Err(e) = server::serve(state_clone).await {
                        tracing::error!("Server error: {}", e);
                    }
                }))
            } else {
                None
            }
        } else {
            None
        };

        // Setup MCP builder and spawn MCP servers
        let mcp_builder = Self::setup_mcp_builder_helper(self.factories, self.mcp_json_path)?;
        let (tool_definitions, mcp_tx, _mcp_handle) = mcp_builder.spawn().await?;

        // Wrap providers in Arc so they can be shared across tasks
        let llm = Arc::new(config.llm);
        let judge_llm = Arc::new(config.judge_llm);

        // Determine batch size (default to all evals if not specified)
        let batch_size = config.batch_size.unwrap_or(evals.len());
        let batch_delay = config.batch_delay.unwrap_or(Duration::ZERO);
        let batch_delay_ms = if batch_delay.is_zero() {
            None
        } else {
            Some(batch_delay.as_millis() as u64)
        };

        let mut run_result = EvalReport::new(
            run_id,
            chrono::Utc::now(),
            config.batch_size,
            batch_delay_ms,
        );
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

            let tasks: Vec<_> = batch
                .into_iter()
                .map(|eval| {
                    Self::spawn_eval_task_helper(
                        eval,
                        agent_prompt.clone(),
                        tool_definitions.clone(),
                        mcp_tx.clone(),
                        llm.clone(),
                        judge_llm.clone(),
                        app_state.clone(),
                    )
                })
                .collect();

            // Await all tasks in this batch concurrently
            let results = futures::future::join_all(tasks).await;

            for result in results {
                Self::handle_eval_result_helper(result, &mut run_result, &results_store, run_id)
                    .await;
            }

            // Update app state after each batch if serving
            if config.serve {
                Self::update_app_state_after_batch_helper(
                    &app_state,
                    &mut run_result,
                    &results_store,
                    run_id,
                )
                .await;
            }

            // Add delay between batches to prevent rate limiting
            if !batch_delay.is_zero() && batch_num * batch_size < total_evals {
                tracing::info!("Waiting {:?} before next batch...", batch_delay);
                tokio::time::sleep(batch_delay).await;
            }
        }

        // Complete the run
        run_result.complete(chrono::Utc::now());

        // Final update to app state if serving
        if config.serve {
            Self::update_app_state_after_batch_helper(
                &app_state,
                &mut run_result,
                &results_store,
                run_id,
            )
            .await;

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

        Ok(run_result)
    }

    // Private helper methods

    /// Set up tracing subscriber with store and fmt layers
    fn setup_tracing_helper(
        run_id: Uuid,
        results_store: &Arc<T>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let env_filter =
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

        let store_layer = results_store.create_tracing_layer(run_id);
        let fmt_layer = tracing_subscriber::fmt::layer().with_writer(std::io::stdout);

        tracing_subscriber::registry()
            .with(store_layer)
            .with(fmt_layer.with_filter(env_filter))
            .try_init()
            .map_err(|e| -> Box<dyn std::error::Error> { e.into() })
    }

    /// Configure MCP builder with factories and json config
    fn setup_mcp_builder_helper(
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
            let repo = git_repo::GitRepo::from_path(path);

            // Capture agent diff (unstaged changes)
            if let Ok(agent_diff_str) = repo.diff_unstaged() {
                let stats = DiffStats::from_diff(&agent_diff_str);
                report.agent_diff = Some(GitDiff {
                    diff: agent_diff_str,
                    stats,
                });
            }

            // Capture reference diff (gold/human solution)
            if let Ok(gold_diff_str) = repo.diff(start_commit, gold_commit) {
                let stats = DiffStats::from_diff(&gold_diff_str);
                report.reference_diff = Some(GitDiff {
                    diff: gold_diff_str,
                    stats,
                });
            }
        }
    }

    /// Spawn a single eval task with tracing instrumentation
    fn spawn_eval_task_helper<M, J>(
        eval: Eval,
        agents_prompt: Option<String>,
        tool_definitions: Vec<aether::llm::ToolDefinition>,
        mcp_tx: tokio::sync::mpsc::Sender<aether::mcp::run_mcp_task::McpCommand>,
        llm: Arc<M>,
        judge_llm: Arc<J>,
        app_state: Option<Arc<server::AppState<T>>>,
    ) -> tokio::task::JoinHandle<(
        Eval,
        Result<Vec<(EvalAssertion, EvalAssertionResult)>, Box<dyn std::error::Error + Send + Sync>>,
        Duration,
        Option<Arc<server::AppState<T>>>,
    )>
    where
        M: StreamingModelProvider + 'static,
        J: StreamingModelProvider + 'static,
    {
        let eval_name = eval.name.clone();
        let eval_name_for_span = eval_name.clone();

        tokio::spawn(
            async move {
                // Broadcast eval started event
                if let Some(state) = &app_state {
                    state.send_sse_event(server::SseEvent::EvalStarted {
                        name: eval_name.clone(),
                    });
                }

                let start = Instant::now();
                let result = eval
                    .run(llm, judge_llm, tool_definitions, mcp_tx, agents_prompt)
                    .await;
                let duration = start.elapsed();
                (eval, result, duration, app_state)
            }
            .instrument(tracing::info_span!("eval_task", eval_name = %eval_name_for_span)),
        )
    }

    /// Handle the result of a single eval task
    async fn handle_eval_result_helper(
        task_result: Result<
            (
                Eval,
                Result<
                    Vec<(EvalAssertion, EvalAssertionResult)>,
                    Box<dyn std::error::Error + Send + Sync>,
                >,
                Duration,
                Option<Arc<server::AppState<T>>>,
            ),
            tokio::task::JoinError,
        >,
        run_result: &mut EvalReport,
        results_store: &Arc<T>,
        run_id: Uuid,
    ) {
        match task_result {
            Ok((eval, Ok(eval_results), _duration, state)) => {
                let mut report = EvalResult::new(&eval, &eval_results[..]);

                // Capture diffs for GitRepo working directories
                Self::capture_git_diffs(&eval, &mut report);

                // Write eval result to store
                if let Err(e) = results_store
                    .save_eval_result(run_id, &eval.name, &report)
                    .await
                {
                    tracing::warn!("Failed to write result file for {}: {}", eval.name, e);
                }

                // Broadcast eval completed event
                if let Some(state) = &state {
                    state.send_sse_event(server::SseEvent::EvalCompleted {
                        name: report.eval_name.clone(),
                        report: report.clone(),
                    });
                }

                run_result.add_eval_result(report);
            }
            Ok((eval, Err(e), _duration, _state)) => {
                tracing::error!("Eval '{}' failed with error: {}", eval.name, e);
            }
            Err(e) => {
                tracing::error!("Task panicked: {}", e);
            }
        }
    }

    /// Update app state with latest run result after batch completion
    async fn update_app_state_after_batch_helper(
        app_state: &Option<Arc<server::AppState<T>>>,
        run_result: &mut EvalReport,
        _results_store: &Arc<T>,
        _run_id: Uuid,
    ) {
        if let Some(state) = app_state {
            state.update_run_result(run_result.clone());
        }
    }
}
