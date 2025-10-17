use aether::{
    agent::{AgentMessage, UserMessage, agent},
    llm::{ChatMessage, Context, StreamingModelProvider, ToolDefinition},
    mcp::run_mcp_task::McpCommand,
    types::IsoString,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};
use tokio::sync::mpsc::Sender;

use futures::StreamExt;

#[derive(Debug, Clone)]
pub struct Eval {
    pub name: String,
    pub prompt: String,
    pub working_dir: PathBuf,
    pub assertions: Vec<EvalAssertion>,
}

impl Eval {
    /// Load an eval from a directory containing prompt.md, optional assertions.json, and optional src/
    pub fn from_path(eval_path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let eval_name = eval_path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| format!("Invalid eval directory name: {:?}", eval_path))?
            .to_string();

        // Read prompt.md
        let prompt_path = eval_path.join("prompt.md");
        let prompt = std::fs::read_to_string(&prompt_path)
            .map_err(|e| format!("Failed to read {:?}: {}", prompt_path, e))?;

        // Create a tmpdir for this eval and copy src/ files into it
        let tmpdir = tempfile::tempdir()
            .map_err(|e| format!("Failed to create tmpdir for {}: {}", eval_name, e))?;

        let src_dir = eval_path.join("src");
        if src_dir.exists() && src_dir.is_dir() {
            copy_dir_all(&src_dir, tmpdir.path())
                .map_err(|e| format!("Failed to copy files from {:?}: {}", src_dir, e))?;
        }

        // Parse assertions.json
        let assertions_path = eval_path.join("assertions.json");
        let assertions = if assertions_path.exists() {
            let content = std::fs::read_to_string(&assertions_path)
                .map_err(|e| format!("Failed to read {:?}: {}", assertions_path, e))?;
            serde_json::from_str::<Vec<EvalAssertion>>(&content)
                .map_err(|e| format!("Failed to parse {:?}: {}", assertions_path, e))?
        } else {
            Vec::new()
        };

        tracing::info!(
            "Loaded eval: {} with {} assertions",
            eval_name,
            assertions.len()
        );

        Ok(Eval {
            name: eval_name,
            prompt,
            working_dir: tmpdir.keep(),
            assertions,
        })
    }

    /// Load all evals from a directory with the expected structure:
    /// ```text
    /// dir/
    ///   evals/
    ///     eval-name-1/
    ///       prompt.md
    ///       assertions.json
    ///       src/  (optional)
    ///     eval-name-2/
    ///       ...
    /// ```
    pub fn load_all(base_dir: &Path) -> Result<Vec<Self>, Box<dyn std::error::Error>> {
        let mut evals = Vec::new();
        let evals_dir = base_dir.join("evals");

        if !evals_dir.exists() {
            return Err(format!("Evals directory not found: {:?}", evals_dir).into());
        }

        let entries = std::fs::read_dir(&evals_dir)
            .map_err(|e| format!("Failed to read evals directory: {}", e))?;

        for entry in entries.flatten() {
            let eval_path = entry.path();
            if !eval_path.is_dir() {
                continue;
            }

            match Self::from_path(&eval_path) {
                Ok(eval) => evals.push(eval),
                Err(e) => {
                    tracing::warn!("Failed to load eval from {:?}: {}, skipping", eval_path, e);
                    continue;
                }
            }
        }

        if evals.is_empty() {
            return Err("No evals found in directory".into());
        }

        Ok(evals)
    }

    pub async fn run<T: StreamingModelProvider + 'static, U: StreamingModelProvider + 'static>(
        &self,
        llm: T,
        judge_llm: U,
        tool_definitions: Vec<ToolDefinition>,
        mcp_tx: Sender<McpCommand>,
        system_prompt: Option<String>,
    ) -> Result<Vec<(EvalAssertion, EvalAssertionResult)>, Box<dyn std::error::Error + Send + Sync>>
    {
        tracing::info!("Running eval: {}", self.name);

        let messages = {
            let mut agent_builder = agent(llm).tools(mcp_tx, tool_definitions);

            if let Some(prompt) = system_prompt {
                agent_builder = agent_builder.system(&prompt);
            }

            let (tx, mut rx, _handle) = agent_builder.spawn().await?;
            let full_prompt = format!(
                "You are working in directory: {}\n\n{}",
                self.working_dir.display(),
                self.prompt
            );

            tx.send(UserMessage::Text {
                content: full_prompt,
            })
            .await?;

            drop(tx);

            let mut messages = Vec::new();
            let mut accumulated_text = String::new();
            let mut accumulated_tool_calls: HashMap<String, aether::llm::ToolCallRequest> =
                HashMap::new();

            while let Some(message) = rx.recv().await {
                let is_done = matches!(message, AgentMessage::Done);

                // Accumulate text chunks and tool calls, only log when complete
                match &message {
                    AgentMessage::Text {
                        chunk, is_complete, ..
                    } => {
                        accumulated_text.push_str(chunk);
                        if *is_complete {
                            tracing::info!("Agent response: {}", accumulated_text);
                            accumulated_text.clear();
                        }
                    }
                    AgentMessage::ToolCall { request, .. } => {
                        let entry = accumulated_tool_calls
                            .entry(request.id.clone())
                            .or_insert_with(|| aether::llm::ToolCallRequest {
                                id: request.id.clone(),
                                name: String::new(),
                                arguments: String::new(),
                            });

                        // Accumulate tool call data
                        if !request.name.is_empty() {
                            entry.name.push_str(&request.name);
                        }
                        entry.arguments.push_str(&request.arguments);

                        // Check if this is a complete tool call (has closing brace for JSON)
                        if !entry.name.is_empty() && entry.arguments.ends_with('}') {
                            tracing::info!(
                                "Tool call: {} with args: {}",
                                entry.name,
                                entry.arguments
                            );
                            accumulated_tool_calls.remove(&request.id);
                        }
                    }
                    AgentMessage::ToolResult { result, .. } => {
                        tracing::info!("Tool result for {}: {}", result.name, result.result);
                    }
                    AgentMessage::ToolError { error, .. } => {
                        tracing::info!("Tool error: {:?}", error);
                    }
                    AgentMessage::Error { message: msg } => {
                        tracing::info!("Agent error: {}", msg);
                    }
                    AgentMessage::Cancelled { message: msg } => {
                        tracing::info!("Agent cancelled: {}", msg);
                    }
                    AgentMessage::Done => {
                        tracing::info!("Agent done");
                    }
                }

                messages.push(message);
                if is_done {
                    break;
                }
            }

            messages
        };

        let mut results = Vec::new();

        for assertion in &self.assertions {
            let result = match assertion {
                EvalAssertion::FileExists { path } => {
                    let file_path = self.working_dir.join(path);
                    if file_path.exists() {
                        tracing::info!("✓ FileExists assertion passed: {}", path);
                        EvalAssertionResult::Success {
                            message: format!("File '{}' exists", path),
                        }
                    } else {
                        tracing::error!("✗ FileExists assertion failed: {}", path);
                        EvalAssertionResult::Failure {
                            message: format!("File '{}' does not exist", path),
                        }
                    }
                }
                EvalAssertion::FileMatches { path, content } => {
                    let file_path = self.working_dir.join(path);
                    match std::fs::read_to_string(&file_path) {
                        Ok(file_content) => {
                            if file_content.contains(content) {
                                tracing::info!("✓ FileMatches assertion passed: {}", path);
                                EvalAssertionResult::Success {
                                    message: format!("File '{}' contains '{}'", path, content),
                                }
                            } else {
                                tracing::error!("✗ FileMatches assertion failed: {}", path);
                                EvalAssertionResult::Failure {
                                    message: format!(
                                        "File '{}' does not contain '{}'",
                                        path, content
                                    ),
                                }
                            }
                        }
                        Err(e) => {
                            tracing::error!("✗ FileMatches assertion failed: {} ({})", path, e);
                            EvalAssertionResult::Failure {
                                message: format!("Failed to read file '{}': {}", path, e),
                            }
                        }
                    }
                }
                EvalAssertion::LLMJudge { prompt } => {
                    tracing::info!("Running LLM judge for assertion");
                    let messages_summary = messages
                        .iter()
                        .map(|m| format!("{:?}", m))
                        .collect::<Vec<_>>()
                        .join("\n");

                    let judge_prompt_text = format!(
                        "You are evaluating an AI agent's performance on a coding task.\n\n\
                         Original Task: {}\n\n\
                         Agent Messages:\n{}\n\n\
                         Evaluation Question: {}\n\n\
                         Respond with either 'SUCCESS: <reason>' or 'FAILURE: <reason>'",
                        self.prompt, messages_summary, prompt
                    );

                    let context = Context::new(
                        vec![ChatMessage::User {
                            content: judge_prompt_text,
                            timestamp: IsoString::now(),
                        }],
                        vec![],
                    );

                    // Call judge LLM directly
                    let mut response_stream = judge_llm.stream_response(&context);

                    let mut judge_response = String::new();
                    while let Some(result) = response_stream.next().await {
                        match result {
                            Ok(llm_response) => {
                                if let aether::llm::LlmResponse::Text { chunk } = llm_response {
                                    judge_response.push_str(&chunk);
                                }
                            }
                            Err(e) => {
                                tracing::error!("✗ LLM judge error: {}", e);
                                return Ok(vec![(
                                    assertion.clone(),
                                    EvalAssertionResult::Failure {
                                        message: format!("Judge LLM error: {}", e),
                                    },
                                )]);
                            }
                        }
                    }

                    // Parse the judge's response
                    if judge_response.to_uppercase().starts_with("SUCCESS") {
                        tracing::info!("✓ LLM judge assertion passed");
                        EvalAssertionResult::Success {
                            message: judge_response,
                        }
                    } else {
                        tracing::error!("✗ LLM judge assertion failed");
                        EvalAssertionResult::Failure {
                            message: judge_response,
                        }
                    }
                }
            };

            results.push((assertion.clone(), result));
        }

        Ok(results)
    }
}

fn copy_dir_all(src: &Path, dst: &Path) -> std::io::Result<()> {
    // Use cp -r to copy directory contents (note the /. to copy contents, not the dir itself)
    let src_with_glob = format!("{}/.  ", src.display());
    let status = std::process::Command::new("cp")
        .arg("-r")
        .arg(src_with_glob.trim())
        .arg(dst)
        .status()?;

    if status.success() {
        Ok(())
    } else {
        Err(std::io::Error::other(format!(
            "Failed to copy directory from {:?} to {:?}",
            src, dst
        )))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum EvalAssertion {
    FileExists { path: String },
    FileMatches { path: String, content: String },
    LLMJudge { prompt: String },
}

impl std::fmt::Display for EvalAssertion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EvalAssertion::FileExists { path } => write!(f, "FileExists({})", path),
            EvalAssertion::FileMatches { path, content } => {
                let truncated = if content.len() > 30 {
                    format!("{}...", &content[..30])
                } else {
                    content.clone()
                };
                write!(f, "FileMatches({}, \"{}\")", path, truncated)
            }
            EvalAssertion::LLMJudge { prompt } => {
                let truncated = if prompt.len() > 40 {
                    format!("{}...", &prompt[..40])
                } else {
                    prompt.clone()
                };
                write!(f, "LLMJudge(\"{}\")", truncated)
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum EvalAssertionResult {
    Success { message: String },
    Failure { message: String },
}

impl EvalAssertionResult {
    pub fn is_success(&self) -> bool {
        matches!(self, EvalAssertionResult::Success { .. })
    }

    pub fn message(&self) -> &str {
        match self {
            EvalAssertionResult::Success { message } => message,
            EvalAssertionResult::Failure { message } => message,
        }
    }

    /// Print the assertion result with the associated assertion
    pub fn print(&self, assertion: &EvalAssertion) {
        use owo_colors::OwoColorize;

        match self {
            EvalAssertionResult::Success { message } => {
                println!(
                    "{} {}: {}",
                    "✓".green().bold(),
                    assertion.to_string().dimmed(),
                    message.green()
                );
            }
            EvalAssertionResult::Failure { message } => {
                println!(
                    "{} {}: {}",
                    "✗".red().bold(),
                    assertion.to_string().dimmed(),
                    message.red()
                );
            }
        }
    }
}
