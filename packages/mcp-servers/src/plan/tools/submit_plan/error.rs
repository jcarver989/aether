use std::fmt::{Display, Formatter};
use std::path::PathBuf;

#[derive(Debug)]
pub(super) enum SubmitPlanError {
    RelativePath,
    MissingFile(PathBuf),
    InvalidUtf8(PathBuf),
    EmptyPlan(PathBuf),
    ReadFailed { path: PathBuf, message: String },
    WorkspaceRootResolution(String),
    CommandSpawn(String),
    CommandFailed { command: String, exit_code: i32, output: String },
    InvalidCommandResponse(String),
    Elicitation(String),
}

impl Display for SubmitPlanError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            SubmitPlanError::RelativePath => {
                write!(f, "planPath must be an absolute path")
            }
            SubmitPlanError::MissingFile(path) => {
                write!(f, "Plan file does not exist: {}", path.display())
            }
            SubmitPlanError::InvalidUtf8(path) => {
                write!(f, "Plan file is not valid UTF-8: {}", path.display())
            }
            SubmitPlanError::EmptyPlan(path) => {
                write!(f, "Plan file is empty: {}", path.display())
            }
            SubmitPlanError::ReadFailed { path, message } => {
                write!(f, "Failed to read plan file {}: {message}", path.display())
            }
            SubmitPlanError::WorkspaceRootResolution(message) => {
                write!(f, "Failed to resolve workspace root: {message}")
            }
            SubmitPlanError::CommandSpawn(message) => {
                write!(f, "Failed to launch external plan reviewer: {message}")
            }
            SubmitPlanError::CommandFailed { command, exit_code, output } => {
                write!(f, "External plan reviewer failed (command='{command}', exit_code={exit_code}):\n{output}")
            }
            SubmitPlanError::InvalidCommandResponse(message) => {
                write!(f, "Invalid external reviewer response: {message}")
            }
            SubmitPlanError::Elicitation(message) => {
                write!(f, "Plan review elicitation failed: {message}")
            }
        }
    }
}
