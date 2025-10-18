pub mod eval;
pub mod eval_assertion;
pub mod eval_messages;
pub mod report;

pub use eval::Eval;
pub use eval_assertion::{EvalAssertion, EvalAssertionResult};
pub use eval_messages::EvalMessage;
pub use report::{AssertionReport, EvalReport, SummaryReport, create_eval_report};

use aether::llm::StreamingModelProvider;
use aether::mcp::{ServerFactory, mcp};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use tracing::Instrument;

pub struct EvalsConfig<T, U> {
    llm: T,
    judge_llm: U,
}

impl<T, U> EvalsConfig<T, U> {
    pub fn new(llm: T, judge_llm: U) -> Self {
        Self { llm, judge_llm }
    }
}

/// Crucible evaluation runner
///
/// Configure and run AI agent evaluations with custom MCP servers
pub struct Crucible {
    base_dir: PathBuf,
    output_dir: Option<PathBuf>,
    factories: HashMap<String, ServerFactory>,
}

impl Crucible {
    /// Create a new Crucible instance
    ///
    /// # Arguments
    /// * `base_dir` - Directory containing AGENTS.md, mcp.json (optional), and evals/
    pub fn new(base_dir: PathBuf) -> Self {
        Self {
            base_dir,
            output_dir: None,
            factories: HashMap::new(),
        }
    }

    /// Set the output directory for logs and results
    ///
    /// If not set, a timestamped directory will be created
    pub fn with_output_dir(mut self, output_dir: PathBuf) -> Self {
        self.output_dir = Some(output_dir);
        self
    }

    /// Register an InMemory MCP server factory
    ///
    /// # Arguments
    /// * `name` - The name of the server (referenced in mcp.json)
    /// * `factory` - Factory function that creates server instances
    pub fn with_server_factory(mut self, name: impl Into<String>, factory: ServerFactory) -> Self {
        self.factories.insert(name.into(), factory);
        self
    }

    /// Register multiple InMemory MCP server factories
    pub fn with_server_factories(mut self, factories: HashMap<String, ServerFactory>) -> Self {
        self.factories.extend(factories);
        self
    }

    /// Load AGENTS.md if it exists in the base directory
    fn load_agents_prompt(&self) -> Option<String> {
        use aether::agent::Prompt;

        let agents_md_path = self.base_dir.join("AGENTS.md");
        if agents_md_path.exists() {
            match Prompt::file(agents_md_path.to_str()?, false).build() {
                Ok(content) => {
                    tracing::info!("Loaded AGENTS.md from {:?}", agents_md_path);
                    Some(content)
                }
                Err(e) => {
                    tracing::warn!("Failed to read AGENTS.md: {}", e);
                    None
                }
            }
        } else {
            tracing::debug!("No AGENTS.md found in {:?}", self.base_dir);
            None
        }
    }

    /// Get the mcp.json path if it exists
    fn mcp_json_path(&self) -> Option<PathBuf> {
        let path = self.base_dir.join("mcp.json");
        if path.exists() { Some(path) } else { None }
    }

    /// Load evals from the base directory
    ///
    /// # Returns
    /// Vector of loaded evals
    pub fn load_evals(&self) -> Result<Vec<Eval>, Box<dyn std::error::Error>> {
        Eval::load_all(&self.base_dir)
    }

    /// Run the evaluations
    ///
    /// # Arguments
    /// * `llm` - The LLM provider to use for running the agent
    /// * `judge_llm` - The LLM provider to use for LLM judge assertions
    ///
    /// # Returns
    /// Result containing the summary report
    pub async fn run_evals<T, U>(
        self,
        config: EvalsConfig<T, U>,
    ) -> Result<SummaryReport, Box<dyn std::error::Error>>
    where
        T: StreamingModelProvider + 'static,
        U: StreamingModelProvider + 'static,
    {
        let evals = self.load_evals()?;
        let agents_prompt = self.load_agents_prompt();
        let mcp_json_path = self.mcp_json_path();

        let output_dir = self.output_dir.unwrap_or_else(|| {
            let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
            PathBuf::from(format!("crucible_output_{}", timestamp))
        });

        std::fs::create_dir_all(&output_dir)?;
        std::fs::create_dir_all(output_dir.join("results"))?;

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
        let tasks: Vec<_> = evals
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

        for task in tasks {
            match task.await {
                Ok((eval, Ok(results), duration)) => {
                    let report = create_eval_report(&eval, &results, Some(duration));

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

        let summary_file = output_dir.join("summary.json");
        summary.write_to_file(&summary_file)?;

        Ok(summary)
    }
}
