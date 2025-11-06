use crate::eval::WorkingDirectory;
use crate::eval_assertion::{EvalAssertionResult, ToolCallCount};
use crate::eval_messages::EvalMessage;
use crate::metrics::EvalMetric;
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
pub fn assert_file_matches(working_dir: &Path, path: &str, content: &str) -> EvalAssertionResult {
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
pub async fn assert_llm_judge<U: StreamingModelProvider, F>(
    working_dir: &WorkingDirectory,
    original_prompt: &str,
    messages: &[EvalMessage],
    prompt_builder: F,
    judge_llm: &U,
) -> EvalAssertionResult
where
    F: Fn(&crate::LlmJudgeContext) -> String,
{
    tracing::info!("Running LLM judge for assertion");

    // Build context for the prompt builder
    let context = crate::LlmJudgeContext {
        working_dir,
        original_prompt,
        messages,
    };

    // Call the prompt builder to get the judge prompt
    let judge_prompt_text = prompt_builder(&context);

    let llm_context = Context::new(
        vec![ChatMessage::User {
            content: judge_prompt_text,
            timestamp: IsoString::now(),
        }],
        vec![],
    );

    let mut response_stream = judge_llm.stream_response(&llm_context);
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

    let trimmed_response = judge_response.trim();
    match serde_json::from_str::<EvalMetric>(trimmed_response) {
        Ok(metric) => {
            let (is_success, reason) = match &metric {
                EvalMetric::Binary { success, reason } => (*success, reason.clone()),
                EvalMetric::Numeric {
                    score,
                    max_score,
                    reason,
                } => {
                    // Consider it a success if score is above 70% of max
                    let success = score / max_score >= 0.7;
                    (success, format!("{reason} (score: {score}/{max_score})"))
                }
            };

            if is_success {
                tracing::info!("✓ LLM judge assertion passed");
                EvalAssertionResult::Success { message: reason }
            } else {
                tracing::error!("✗ LLM judge assertion failed");
                EvalAssertionResult::Failure { message: reason }
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
            message: format!("Tool '{name}' was called {actual_count} time(s) successfully"),
        }
    }
}
