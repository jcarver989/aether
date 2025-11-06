mod assertions;
pub mod eval;
pub mod eval_assertion;
pub mod eval_messages;
pub mod git_repo;
pub mod report;

pub use eval::{Eval, WorkingDirectory};
pub use eval_assertion::{EvalAssertion, EvalAssertionResult, ToolCallCount};
pub use eval_messages::EvalMessage;
pub use report::{
    AssertionReport, EvalReport, ReportData, SummaryReport, copy_report_templates,
    create_eval_report, serve_report, update_report_data,
};

use aether::llm::StreamingModelProvider;
use aether::mcp::{ServerFactory, mcp};
use owo_colors::OwoColorize;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::Instrument;

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
pub struct EvalRunner {
    output_dir: Option<PathBuf>,
    factories: HashMap<String, ServerFactory>,
    agent_prompt: Option<String>,
    mcp_json_path: Option<PathBuf>,
}

impl EvalRunner {
    /// Create a new Crucible instance
    pub fn new() -> Self {
        Self {
            output_dir: None,
            factories: HashMap::new(),
            agent_prompt: None,
            mcp_json_path: None,
        }
    }

    /// Set the output directory for logs and results
    ///
    /// If not set, a timestamped directory will be created
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
    pub fn with_mcp_json(mut self, path: PathBuf) -> Self {
        self.mcp_json_path = Some(path);
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
    pub async fn run_evals<T, U>(
        self,
        evals: Vec<Eval>,
        config: EvalsConfig<T, U>,
    ) -> Result<SummaryReport, Box<dyn std::error::Error>>
    where
        T: StreamingModelProvider + 'static,
        U: StreamingModelProvider + 'static,
    {
        if evals.is_empty() {
            return Err("No evals provided".into());
        }

        let agents_prompt = self.agent_prompt;
        let mcp_json_path = self.mcp_json_path;

        let output_dir = self.output_dir.unwrap_or_else(|| {
            let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
            PathBuf::from(format!("crucible_output_{timestamp}"))
        });

        std::fs::create_dir_all(&output_dir)?;
        std::fs::create_dir_all(output_dir.join("results"))?;
        std::fs::create_dir_all(output_dir.join("report"))?;

        // Set up tracing to write to both stdout and traces.jsonl
        let traces_file = output_dir.join("traces.jsonl");
        let file_appender = tracing_appender::rolling::never(&output_dir, "traces.jsonl");

        use tracing_subscriber::layer::SubscriberExt;
        use tracing_subscriber::util::SubscriberInitExt;

        // Create a JSON layer for file output
        let json_layer = tracing_subscriber::fmt::layer()
            .json()
            .with_writer(file_appender);

        // Create a formatted layer for stdout
        let fmt_layer = tracing_subscriber::fmt::layer().with_writer(std::io::stdout);

        // Try to set as global default (will fail silently if already initialized)
        let _result = tracing_subscriber::registry()
            .with(json_layer)
            .with(fmt_layer)
            .try_init();

        // Start web server in background if requested
        let _server_handle = if config.serve {
            // Create initial empty report data
            let empty_report = report::ReportData {
                summary: SummaryReport::new(),
                eval_traces: HashMap::new(),
            };
            let empty_json = serde_json::to_string_pretty(&empty_report)?;
            std::fs::write(
                output_dir.join("report").join("report-data.json"),
                empty_json,
            )?;

            // Spawn server in background task
            let output_dir_clone = output_dir.clone();
            Some(tokio::spawn(async move {
                if let Err(e) = serve_report(&output_dir_clone) {
                    tracing::error!("Server error: {}", e);
                }
            }))
        } else {
            None
        };

        let mut mcp_builder = mcp();
        for (name, factory) in self.factories {
            mcp_builder = mcp_builder.register_in_memory_server(name, factory);
        }

        if let Some(mcp_json_path) = mcp_json_path {
            mcp_builder = mcp_builder.from_json_file(mcp_json_path.to_str().unwrap())?;
        }

        let (tool_definitions, mcp_tx, _mcp_handle) = mcp_builder.spawn().await?;

        // Wrap providers in Arc so they can be shared across tasks
        let llm = Arc::new(config.llm);
        let judge_llm = Arc::new(config.judge_llm);

        let mut summary = SummaryReport::new();

        // Determine batch size (default to all evals if not specified)
        let batch_size = config.batch_size.unwrap_or(evals.len());
        let batch_delay = config.batch_delay.unwrap_or(Duration::ZERO);

        // Process evals in batches
        for batch_start in (0..evals.len()).step_by(batch_size) {
            let batch_end = std::cmp::min(batch_start + batch_size, evals.len());
            let batch: Vec<Eval> = evals[batch_start..batch_end].to_vec();

            tracing::info!(
                "Processing batch {}/{} ({} evals)",
                (batch_start / batch_size) + 1,
                evals.len().div_ceil(batch_size),
                batch.len()
            );

            let tasks: Vec<_> = batch
                .into_iter()
                .map(|eval| {
                    let agents_prompt_clone = agents_prompt.clone();
                    let tool_definitions_clone = tool_definitions.clone();
                    let mcp_tx_clone = mcp_tx.clone();
                    let llm_clone = llm.clone();
                    let judge_llm_clone = judge_llm.clone();
                    let eval_name = eval.name.clone();

                    tokio::spawn(
                        async move {
                            let start = Instant::now();

                            let result = eval
                                .run(
                                    llm_clone,
                                    judge_llm_clone,
                                    tool_definitions_clone,
                                    mcp_tx_clone,
                                    agents_prompt_clone,
                                )
                                .await;
                            let duration = start.elapsed();
                            (eval, result, duration)
                        }
                        .instrument(tracing::info_span!("eval_task", eval_name = %eval_name)),
                    )
                })
                .collect();

            // Await all tasks in this batch concurrently
            let results = futures::future::join_all(tasks).await;

            for result in results {
                match result {
                    Ok((eval, Ok(eval_results), duration)) => {
                        let report = create_eval_report(&eval, &eval_results, Some(duration));

                        let result_file = output_dir
                            .join("results")
                            .join(format!("{}.json", eval.name));
                        if let Err(e) = report.write_to_file(&result_file) {
                            tracing::warn!("Failed to write result file for {}: {}", eval.name, e);
                        }

                        summary.add_eval(report);
                    }
                    Ok((eval, Err(e), _duration)) => {
                        tracing::error!("Eval '{}' failed with error: {}", eval.name, e);
                    }
                    Err(e) => {
                        tracing::error!("Task panicked: {}", e);
                    }
                }
            }

            // Update report data after each batch if serving
            if config.serve {
                if let Err(e) = update_report_data(&output_dir, &summary, &traces_file) {
                    tracing::warn!("Failed to update report data: {}", e);
                }
            }

            // Add delay between batches to prevent rate limiting
            if !batch_delay.is_zero() && batch_end < evals.len() {
                tracing::info!("Waiting {:?} before next batch...", batch_delay);
                tokio::time::sleep(batch_delay).await;
            }
        }

        let summary_file = output_dir.join("summary.json");
        summary.write_to_file(&summary_file)?;

        // Final update of report data
        if config.serve {
            if let Err(e) = update_report_data(&output_dir, &summary, &traces_file) {
                tracing::warn!("Failed to update final report data: {}", e);
            }

            // Keep the server running (it's in a background task)
            println!(
                "\n{}",
                "Server is still running. Press Ctrl+C to exit."
                    .bold()
                    .green()
            );
            // Wait indefinitely - user must Ctrl+C to exit
            tokio::signal::ctrl_c().await?;
        }

        Ok(summary)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    // Mock LLM provider for testing
    struct MockLLM;

    impl aether::llm::StreamingModelProvider for MockLLM {
        fn stream_response(
            &self,
            _context: &aether::llm::Context,
        ) -> aether::llm::LlmResponseStream {
            Box::pin(futures::stream::empty())
        }

        fn display_name(&self) -> String {
            "MockLLM".to_string()
        }
    }

    #[test]
    fn test_evals_config_batch_configuration() {
        let llm = MockLLM;
        let judge_llm = MockLLM;

        // Test default configuration
        let config = EvalsConfig::new(llm, judge_llm);
        assert!(config.batch_size.is_none());
        assert!(config.batch_delay.is_none());

        // Test with batch size
        let llm = MockLLM;
        let judge_llm = MockLLM;
        let config = EvalsConfig::new(llm, judge_llm)
            .with_batch_size(5)
            .with_batch_delay(Duration::from_millis(1000));

        assert_eq!(config.batch_size, Some(5));
        assert_eq!(config.batch_delay, Some(Duration::from_millis(1000)));
    }
}
