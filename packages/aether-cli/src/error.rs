use std::fmt::Display;
use std::io;

#[derive(Debug)]
pub enum CliError {
    NoPrompt,
    ModelError(String),
    McpError(String),
    IoError(io::Error),
    AgentError(String),
}

impl Display for CliError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoPrompt => write!(
                f,
                "No prompt provided. Pass a prompt as an argument or pipe via stdin."
            ),
            Self::ModelError(e) => write!(f, "Model error: {e}"),
            Self::McpError(e) => write!(f, "MCP error: {e}"),
            Self::IoError(e) => write!(f, "IO error: {e}"),
            Self::AgentError(e) => write!(f, "Agent error: {e}"),
        }
    }
}

impl std::error::Error for CliError {}
