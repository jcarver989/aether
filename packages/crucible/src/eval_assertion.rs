use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ToolCallCount {
    Exact(usize),
    AtLeast(usize),
    AtMost(usize),
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
                    "CommandExitCode(\"{}\", code={})",
                    truncated, expected_code
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
                    format!(" {:?}", cnt)
                } else {
                    "".to_string()
                };

                write!(f, "ToolCall({}, args={}{})", name, args_str, count_str)
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
