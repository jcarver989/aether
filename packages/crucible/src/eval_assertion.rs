use crate::git_repo::GitRepo;
use crate::{EvalMessage, WorkingDirectory};
use std::sync::Arc;

/// Context provided to LLM judge prompt builders
pub struct LlmJudgeContext<'a> {
    pub working_dir: &'a WorkingDirectory,
    pub original_prompt: &'a str,
    pub messages: &'a [EvalMessage],
}

impl<'a> LlmJudgeContext<'a> {
    /// Get a git diff between the start commit and an optional end commit
    ///
    /// If `to_commit` is None, returns unstaged changes in the working directory.
    /// If `to_commit` is Some, returns the diff between start and that commit.
    pub fn git_diff(&self, to_commit: Option<&str>) -> Option<String> {
        match self.working_dir {
            WorkingDirectory::GitRepo {
                path, start_commit, ..
            } => {
                let git_repo = GitRepo::from_path(path);
                match to_commit {
                    Some(commit) => git_repo.diff(start_commit, commit).ok(),
                    None => git_repo.diff_unstaged().ok(),
                }
            }
            WorkingDirectory::Local { .. } => None,
        }
    }
}

/// Assertions for evaluating agent behavior
#[derive(Clone)]
pub enum EvalAssertion {
    FileExists {
        path: String,
    },
    FileMatches {
        path: String,
        content: String,
    },
    LLMJudge {
        prompt_builder: Arc<dyn Fn(&LlmJudgeContext) -> String + Send + Sync>,
    },
    CommandExitCode {
        command: String,
        expected_code: i32,
    },
    ToolCall {
        name: String,
        arguments: Option<serde_json::Value>,
        count: Option<ToolCallCount>,
    },
}

#[derive(Debug, Clone)]
pub enum ToolCallCount {
    Exact(usize),
    AtLeast(usize),
    AtMost(usize),
}

// Builder methods for creating assertions programmatically
impl EvalAssertion {
    /// Assert that a file exists at the given path
    pub fn file_exists(path: impl Into<String>) -> Self {
        Self::FileExists { path: path.into() }
    }

    /// Assert that a file exists and contains the given content
    pub fn file_matches(path: impl Into<String>, content: impl Into<String>) -> Self {
        Self::FileMatches {
            path: path.into(),
            content: content.into(),
        }
    }

    /// Use an LLM to judge whether the agent succeeded
    ///
    /// The prompt_builder function receives context about the eval and returns
    /// a prompt string that will be sent to the judge LLM.
    pub fn llm_judge<F>(prompt_builder: F) -> Self
    where
        F: Fn(&LlmJudgeContext) -> String + Send + Sync + 'static,
    {
        Self::LLMJudge {
            prompt_builder: Arc::new(prompt_builder),
        }
    }

    /// Assert that a command exits with the expected code
    pub fn command_exit_code(command: impl Into<String>, expected_code: i32) -> Self {
        Self::CommandExitCode {
            command: command.into(),
            expected_code,
        }
    }

    /// Assert that a command succeeds (exit code 0)
    pub fn command_succeeds(command: impl Into<String>) -> Self {
        Self::CommandExitCode {
            command: command.into(),
            expected_code: 0,
        }
    }

    /// Assert that a specific tool was called
    pub fn tool_call(name: impl Into<String>) -> Self {
        Self::ToolCall {
            name: name.into(),
            arguments: None,
            count: None,
        }
    }

    /// Assert that a tool was called with specific arguments
    pub fn tool_call_with_args(name: impl Into<String>, arguments: serde_json::Value) -> Self {
        Self::ToolCall {
            name: name.into(),
            arguments: Some(arguments),
            count: None,
        }
    }

    /// Assert that a tool was called an exact number of times
    pub fn tool_call_exact(name: impl Into<String>, count: usize) -> Self {
        Self::ToolCall {
            name: name.into(),
            arguments: None,
            count: Some(ToolCallCount::Exact(count)),
        }
    }

    /// Assert that a tool was called at least N times
    pub fn tool_call_at_least(name: impl Into<String>, count: usize) -> Self {
        Self::ToolCall {
            name: name.into(),
            arguments: None,
            count: Some(ToolCallCount::AtLeast(count)),
        }
    }

    /// Assert that a tool was called at most N times
    pub fn tool_call_at_most(name: impl Into<String>, count: usize) -> Self {
        Self::ToolCall {
            name: name.into(),
            arguments: None,
            count: Some(ToolCallCount::AtMost(count)),
        }
    }
}

impl std::fmt::Debug for EvalAssertion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EvalAssertion::FileExists { path } => {
                f.debug_struct("FileExists").field("path", path).finish()
            }
            EvalAssertion::FileMatches { path, content } => f
                .debug_struct("FileMatches")
                .field("path", path)
                .field("content", content)
                .finish(),
            EvalAssertion::LLMJudge { .. } => f
                .debug_struct("LLMJudge")
                .field("prompt_builder", &"<function>")
                .finish(),
            EvalAssertion::CommandExitCode {
                command,
                expected_code,
            } => f
                .debug_struct("CommandExitCode")
                .field("command", command)
                .field("expected_code", expected_code)
                .finish(),
            EvalAssertion::ToolCall {
                name,
                arguments,
                count,
            } => f
                .debug_struct("ToolCall")
                .field("name", name)
                .field("arguments", arguments)
                .field("count", count)
                .finish(),
        }
    }
}

impl std::fmt::Display for EvalAssertion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EvalAssertion::FileExists { path } => write!(f, "FileExists({path})"),
            EvalAssertion::FileMatches { path, content } => {
                let truncated = if content.len() > 30 {
                    format!("{}...", &content[..30])
                } else {
                    content.clone()
                };
                write!(f, "FileMatches({path}, \"{truncated}\")")
            }
            EvalAssertion::LLMJudge { .. } => {
                write!(f, "LLMJudge(<custom>)")
            }
            EvalAssertion::CommandExitCode {
                command,
                expected_code,
            } => {
                let truncated = if command.len() > 40 {
                    format!("{}...", &command[..40])
                } else {
                    command.clone()
                };
                write!(f, "CommandExitCode(\"{truncated}\", code={expected_code})")
            }
            EvalAssertion::ToolCall {
                name,
                arguments,
                count,
            } => {
                let args_str = if let Some(args) = arguments {
                    let args_json = serde_json::to_string(args).unwrap_or_default();
                    if args_json.len() > 30 {
                        format!("{}...", &args_json[..30])
                    } else {
                        args_json
                    }
                } else {
                    "any".to_string()
                };

                let count_str = if let Some(cnt) = count {
                    format!(" {cnt:?}")
                } else {
                    "".to_string()
                };

                write!(f, "ToolCall({name}, args={args_str}{count_str})")
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
