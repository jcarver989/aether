use aether::{
    agent::{UserMessage, agent},
    llm::{ChatMessage, Context, StreamingModelProvider, ToolDefinition},
    mcp::run_mcp_task::McpCommand,
    types::IsoString,
};
use std::path::{Path, PathBuf};
use tokio::sync::mpsc::Sender;

use crate::eval_assertion::{EvalAssertion, EvalAssertionResult, ToolCallCount};
use crate::eval_messages::{EvalMessage, to_eval_messages};
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
            .ok_or_else(|| format!("Invalid eval directory name: {eval_path:?}"))?
            .to_string();

        // Read prompt.md
        let prompt_path = eval_path.join("prompt.md");
        let prompt = std::fs::read_to_string(&prompt_path)
            .map_err(|e| format!("Failed to read {prompt_path:?}: {e}"))?;

        // Create a tmpdir for this eval and copy src/ files into it
        let tmpdir = tempfile::tempdir()
            .map_err(|e| format!("Failed to create tmpdir for {eval_name}: {e}"))?;

        let src_dir = eval_path.join("src");
        if src_dir.exists() && src_dir.is_dir() {
            copy_dir_all(&src_dir, tmpdir.path())
                .map_err(|e| format!("Failed to copy files from {src_dir:?}: {e}"))?;
        }

        // Parse assertions.json
        let assertions_path = eval_path.join("assertions.json");
        let assertions = if assertions_path.exists() {
            let content = std::fs::read_to_string(&assertions_path)
                .map_err(|e| format!("Failed to read {assertions_path:?}: {e}"))?;
            serde_json::from_str::<Vec<EvalAssertion>>(&content)
                .map_err(|e| format!("Failed to parse {assertions_path:?}: {e}"))?
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
            return Err(format!("Evals directory not found: {evals_dir:?}").into());
        }

        let entries = std::fs::read_dir(&evals_dir)
            .map_err(|e| format!("Failed to read evals directory: {e}"))?;

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
        let span = tracing::info_span!("eval", eval_name = %self.name);
        let _enter = span.enter();

        tracing::info!("Running eval: {}", self.name);

        let messages = {
            let mut agent_builder = agent(llm).tools(mcp_tx, tool_definitions);

            if let Some(prompt) = system_prompt {
                agent_builder = agent_builder.system(&prompt);
            }

            let (tx, rx, _handle) = agent_builder.spawn().await?;

            tx.send(UserMessage::Text {
                content: [
                    self.prompt.to_string(),
                    format!("CRITICAL INSTRUCTIONS: when working on this task, you MUST only operate within this directory: {}", self.working_dir.display())].join("\n"),
            })
            .await?;
            to_eval_messages(rx).await
        };

        let mut results = Vec::new();

        for assertion in &self.assertions {
            let result = match assertion {
                EvalAssertion::FileExists { path } => self.assert_file_exists(path),
                EvalAssertion::FileMatches { path, content } => {
                    self.assert_file_matches(path, content)
                }
                EvalAssertion::CommandExitCode {
                    command,
                    expected_code,
                } => self.assert_command_exit_code(command, *expected_code).await,
                EvalAssertion::LLMJudge { prompt } => {
                    self.assert_llm_judge(&messages, prompt, &judge_llm).await
                }
                EvalAssertion::ToolCall {
                    name,
                    arguments,
                    count,
                } => {
                    self.assert_tool_call(name, arguments.as_ref(), count, &messages)
                        .await
                }
            };

            results.push((assertion.clone(), result));
        }

        Ok(results)
    }

    /// Check if a file exists at the specified path
    fn assert_file_exists(&self, path: &str) -> EvalAssertionResult {
        let file_path = self.working_dir.join(path);
        if file_path.exists() {
            tracing::info!("✓ FileExists assertion passed: {}", path);
            EvalAssertionResult::Success {
                message: format!("File '{path}' exists"),
            }
        } else {
            tracing::error!("✗ FileExists assertion failed: {}", path);
            EvalAssertionResult::Failure {
                message: format!("File '{path}' does not exist"),
            }
        }
    }

    /// Check if a file contains specific content
    fn assert_file_matches(&self, path: &str, content: &str) -> EvalAssertionResult {
        let file_path = self.working_dir.join(path);
        match std::fs::read_to_string(&file_path) {
            Ok(file_content) => {
                if file_content.contains(content) {
                    tracing::info!("✓ FileMatches assertion passed: {}", path);
                    EvalAssertionResult::Success {
                        message: format!("File '{path}' contains '{content}'"),
                    }
                } else {
                    tracing::error!("✗ FileMatches assertion failed: {}", path);
                    EvalAssertionResult::Failure {
                        message: format!("File '{path}' does not contain '{content}'"),
                    }
                }
            }
            Err(e) => {
                tracing::error!("✗ FileMatches assertion failed: {} ({})", path, e);
                EvalAssertionResult::Failure {
                    message: format!("Failed to read file '{path}': {e}"),
                }
            }
        }
    }

    /// Check if a command exits with the expected code
    async fn assert_command_exit_code(
        &self,
        command: &str,
        expected_code: i32,
    ) -> EvalAssertionResult {
        tracing::info!("Running command: {}", command);

        let output = tokio::process::Command::new("sh")
            .arg("-c")
            .arg(command)
            .current_dir(&self.working_dir)
            .output()
            .await;

        match output {
            Ok(output) => {
                let actual_code = output.status.code().unwrap_or(-1);
                if actual_code == expected_code {
                    tracing::info!(
                        "✓ CommandExitCode assertion passed: {} (exit code: {})",
                        command,
                        actual_code
                    );
                    EvalAssertionResult::Success {
                        message: format!(
                            "Command '{command}' exited with code {actual_code} as expected"
                        ),
                    }
                } else {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    tracing::error!(
                        "✗ CommandExitCode assertion failed: {} (expected: {}, got: {})",
                        command,
                        expected_code,
                        actual_code
                    );
                    EvalAssertionResult::Failure {
                        message: format!(
                            "Command '{command}' exited with code {actual_code} (expected {expected_code})\nstderr: {stderr}"
                        ),
                    }
                }
            }
            Err(e) => {
                tracing::error!("✗ CommandExitCode assertion failed: {} ({})", command, e);
                EvalAssertionResult::Failure {
                    message: format!("Failed to execute command '{command}': {e}"),
                }
            }
        }
    }

    /// Check an assertion using the LLM judge
    async fn assert_llm_judge<U: StreamingModelProvider>(
        &self,
        messages: &[EvalMessage],
        prompt: &str,
        judge_llm: &U,
    ) -> EvalAssertionResult {
        tracing::info!("Running LLM judge for assertion");

        let mut messages_summary = String::new();
        for msg in messages {
            match msg {
                EvalMessage::AgentText(text) => {
                    messages_summary.push_str("Agent: ");
                    messages_summary.push_str(text);
                    messages_summary.push('\n');
                }
                EvalMessage::ToolCall { name, arguments } => {
                    messages_summary.push_str(&format!("Tool call: {name} ({arguments})\n"));
                }
                EvalMessage::ToolResult { name, result } => {
                    messages_summary.push_str(&format!("Tool result ({name}): {result}\n"));
                }
                EvalMessage::ToolError(error) => {
                    messages_summary.push_str(&format!("Tool error: {error}\n"));
                }
                EvalMessage::Error(error) => {
                    messages_summary.push_str(&format!("Error: {error}\n"));
                }
                EvalMessage::Done => {}
            }
        }

        let judge_prompt_text = format!(
            "You are evaluating an AI agent's performance on a coding task. The agent was asked to perform all work in this directory: {}\n\n\
             Original Task: {}\n\n\
             Agent Messages:\n{}\n\n\
             Evaluation Question: {}\n\n\
             Respond with valid JSON in this exact format:\n\
             {{\"success\": true, \"reason\": \"explanation\"}} for success\n\
             {{\"success\": false, \"reason\": \"explanation\"}} for failure\n\n\
             Only output the JSON, nothing else.",
            self.working_dir.display(),
            self.prompt,
            messages_summary,
            prompt
        );

        let context = Context::new(
            vec![ChatMessage::User {
                content: judge_prompt_text,
                timestamp: IsoString::now(),
            }],
            vec![],
        );

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
                    return EvalAssertionResult::Failure {
                        message: format!("Judge LLM error: {e}"),
                    };
                }
            }
        }

        // Parse the judge's response as JSON
        #[derive(serde::Deserialize)]
        struct JudgeResponse {
            success: bool,
            reason: String,
        }

        let trimmed_response = judge_response.trim();
        match serde_json::from_str::<JudgeResponse>(trimmed_response) {
            Ok(parsed) => {
                if parsed.success {
                    tracing::info!("✓ LLM judge assertion passed");
                    EvalAssertionResult::Success {
                        message: parsed.reason,
                    }
                } else {
                    tracing::error!("✗ LLM judge assertion failed");
                    EvalAssertionResult::Failure {
                        message: parsed.reason,
                    }
                }
            }
            Err(e) => {
                tracing::error!("✗ LLM judge returned invalid JSON: {}", e);
                tracing::error!("Raw response: {}", judge_response);
                EvalAssertionResult::Failure {
                    message: format!(
                        "Judge returned invalid JSON: {e}\nRaw response: {judge_response}"
                    ),
                }
            }
        }
    }

    /// Check if a tool was called with matching arguments
    async fn assert_tool_call(
        &self,
        name: &str,
        expected_args: Option<&serde_json::Value>,
        count: &Option<ToolCallCount>,
        messages: &[EvalMessage],
    ) -> EvalAssertionResult {
        let matching_calls: Vec<_> = messages
            .iter()
            .filter_map(|msg| {
                if let EvalMessage::ToolCall {
                    name: call_name,
                    arguments,
                } = msg
                {
                    if call_name != name {
                        return None;
                    }

                    let actual_args = match serde_json::from_str::<serde_json::Value>(arguments) {
                        Ok(args) => args,
                        Err(_) => return None, // Invalid JSON
                    };

                    match expected_args {
                        Some(expected) if actual_args == *expected => Some(actual_args),
                        None => Some(actual_args), // No arg matching required
                        _ => None,                 // Args don't match
                    }
                } else {
                    None
                }
            })
            .collect();

        let actual_count = matching_calls.len();

        if let Some(count_req) = count {
            let count_valid = match count_req {
                ToolCallCount::Exact(expected) => actual_count == *expected,
                ToolCallCount::AtLeast(min) => actual_count >= *min,
                ToolCallCount::AtMost(max) => actual_count <= *max,
            };

            if !count_valid {
                return EvalAssertionResult::Failure {
                    message: format!(
                        "Tool '{name}' was called {actual_count} times, but expected {count_req:?}"
                    ),
                };
            }
        }

        if matching_calls.is_empty() {
            EvalAssertionResult::Failure {
                message: format!("Tool '{name}' was not called with matching arguments"),
            }
        } else {
            tracing::info!(
                "✓ ToolCall assertion passed: {} (matched {} time(s))",
                name,
                actual_count
            );
            EvalAssertionResult::Success {
                message: format!(
                    "Tool '{name}' was called {actual_count} time(s) successfully"
                ),
            }
        }
    }
}

fn copy_dir_all(src: &Path, dst: &Path) -> std::io::Result<()> {
    // Keep the directory structure (e.g., src/ -> dst/src/)
    let status = std::process::Command::new("cp")
        .arg("-r")
        .arg(src)
        .arg(dst)
        .status()?;

    if status.success() {
        Ok(())
    } else {
        Err(std::io::Error::other(format!(
            "Failed to copy directory from {src:?} to {dst:?}"
        )))
    }
}
