use serde::{Deserialize, Serialize};
use std::time::Duration;

use crate::{Eval, EvalAssertion, EvalAssertionResult};

/// The result of running a single evaluation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalResult {
    pub eval_name: String,
    pub passed: bool,
    pub duration: Option<Duration>,
    pub assertions: Vec<AssertionResult>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_diff: Option<GitDiff>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference_diff: Option<GitDiff>,
}

impl EvalResult {
    pub fn new(
        eval: &Eval,
        results: &[(EvalAssertion, EvalAssertionResult)],
        duration: Option<Duration>,
    ) -> EvalResult {
        let assertions: Vec<AssertionResult> = results
            .iter()
            .map(|(assertion, result)| AssertionResult {
                assertion_type: match assertion {
                    EvalAssertion::FileExists { .. } => "FileExists".to_string(),
                    EvalAssertion::FileMatches { .. } => "FileMatches".to_string(),
                    EvalAssertion::LLMJudge { .. } => "LLMJudge".to_string(),
                    EvalAssertion::CommandExitCode { .. } => "CommandExitCode".to_string(),
                    EvalAssertion::ToolCall { .. } => "ToolCall".to_string(),
                },
                passed: result.is_success(),
                message: result.message().to_string(),
            })
            .collect();

        let passed = assertions.iter().all(|a| a.passed);

        EvalResult {
            eval_name: eval.name.clone(),
            passed,
            duration,
            assertions,
            agent_diff: None,
            reference_diff: None,
        }
    }

    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssertionResult {
    pub assertion_type: String,
    pub passed: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitDiff {
    pub diff: String,
    pub stats: DiffStats,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffStats {
    pub files_changed: usize,
    pub lines_added: usize,
    pub lines_removed: usize,
}

impl DiffStats {
    /// Compute basic diff statistics from a git diff string
    pub fn from_diff(diff: &str) -> Self {
        let mut lines_added = 0;
        let mut lines_removed = 0;
        let mut files_changed = 0;

        for line in diff.lines() {
            if line.starts_with("diff --git") {
                files_changed += 1;
            } else if line.starts_with('+') && !line.starts_with("+++") {
                lines_added += 1;
            } else if line.starts_with('-') && !line.starts_with("---") {
                lines_removed += 1;
            }
        }

        Self {
            files_changed,
            lines_added,
            lines_removed,
        }
    }
}
