/// Assertions for evaluating agent behavior
#[derive(Debug, Clone)]
pub enum EvalAssertion {
    FileExists {
        path: String,
    },
    FileMatches {
        path: String,
        content: String,
    },
    LLMJudge {
        prompt: String,
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
    pub fn llm_judge(prompt: impl Into<String>) -> Self {
        Self::LLMJudge {
            prompt: prompt.into(),
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
    pub fn tool_call_with_args(
        name: impl Into<String>,
        arguments: serde_json::Value,
    ) -> Self {
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
            EvalAssertion::LLMJudge { prompt } => {
                let truncated = if prompt.len() > 40 {
                    format!("{}...", &prompt[..40])
                } else {
                    prompt.clone()
                };
                write!(f, "LLMJudge(\"{truncated}\")")
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
                write!(
                    f,
                    "CommandExitCode(\"{truncated}\", code={expected_code})"
                )
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
