use crate::eval::WorkingDirectory;
use crate::eval_assertion::{EvalAssertionResult, ToolCallCount};
use crate::eval_messages::EvalMessage;
use crate::git_repo::GitRepo;
use aether::llm::{ChatMessage, Context, StreamingModelProvider};
use aether::types::IsoString;
use futures::StreamExt;
use std::path::Path;

/// Check if a file exists at the specified path
pub fn assert_file_exists(working_dir: &Path, path: &str) -> EvalAssertionResult {
    let file_path = working_dir.join(path);
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
pub fn assert_file_matches(
    working_dir: &Path,
    path: &str,
    content: &str,
) -> EvalAssertionResult {
    let file_path = working_dir.join(path);
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
pub async fn assert_command_exit_code(
    working_dir: &Path,
    command: &str,
    expected_code: i32,
) -> EvalAssertionResult {
    tracing::info!("Running command: {}", command);

    let output = tokio::process::Command::new("sh")
        .arg("-c")
        .arg(command)
        .current_dir(working_dir)
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
pub async fn assert_llm_judge<U: StreamingModelProvider>(
    working_dir: &WorkingDirectory,
    original_prompt: &str,
    messages: &[EvalMessage],
    judge_prompt: &str,
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

    // Build git context if available
    let git_context = match working_dir {
        WorkingDirectory::GitRepo {
            path,
            url,
            start_commit,
            gold_commit,
        } => {
            // Generate git diff between start and gold commits
            let git_repo = GitRepo::from_path(path);
            let diff_result = git_repo.diff(start_commit, gold_commit);

            match diff_result {
                Ok(diff) => {
                    // Check if diff is too large (> 50k chars)
                    const MAX_DIFF_SIZE: usize = 50_000;
                    let diff_display = if diff.len() > MAX_DIFF_SIZE {
                        tracing::warn!(
                            "Git diff is too large ({} chars), truncating to {} chars",
                            diff.len(),
                            MAX_DIFF_SIZE
                        );
                        format!(
                            "{}\n\n[... diff truncated, showing first {} of {} characters ...]",
                            &diff[..MAX_DIFF_SIZE],
                            MAX_DIFF_SIZE,
                            diff.len()
                        )
                    } else {
                        diff
                    };

                    Some(format!(
                        "\n\nGit Repository Context:\n\
                         - Repository: {}\n\
                         - Start Commit (agent started from): {}\n\
                         - Gold Commit (target solution): {}\n\
                         - Actual Changes (git diff):\n\
                         ```\n\
                         {}\n\
                         ```\n",
                        url, start_commit, gold_commit, diff_display
                    ))
                }
                Err(e) => {
                    tracing::warn!("Failed to generate git diff: {}", e);
                    Some(format!(
                        "\n\nGit Repository Context:\n\
                         - Repository: {}\n\
                         - Start Commit: {}\n\
                         - Gold Commit: {}\n\
                         - Note: Failed to generate diff: {}\n",
                        url, start_commit, gold_commit, e
                    ))
                }
            }
        }
        WorkingDirectory::Local { .. } => None,
    };

    let judge_prompt_text = format!(
        "You are evaluating an AI agent's performance on a coding task. The agent was asked to perform all work in this directory: {}\n\n\
         Original Task: {}\n\n\
         Agent Messages:\n{}\n{}\
         Evaluation Question: {}\n\n\
         Respond with valid JSON in this exact format:\n\
         {{\"success\": true, \"reason\": \"explanation\"}} for success\n\
         {{\"success\": false, \"reason\": \"explanation\"}} for failure\n\n\
         Only output the JSON, nothing else.",
        working_dir.path().display(),
        original_prompt,
        messages_summary,
        git_context.as_deref().unwrap_or(""),
        judge_prompt
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
pub async fn assert_tool_call(
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
