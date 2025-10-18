use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum EvalAssertion {
    FileExists { path: String },
    FileMatches { path: String, content: String },
    LLMJudge { prompt: String },
    CommandExitCode { command: String, expected_code: i32 },
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