use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    Eval, EvalAssertion,
    eval_assertion::EvalAssertionResult as EvalAssertionResultEnum,
};

/// Result of running a single evaluation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalResult {
    pub id: Uuid,
    pub eval_name: String,
    pub passed: bool,
    pub assertions: Vec<EvalAssertionResult>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_diff: Option<GitDiff>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference_diff: Option<GitDiff>,
}

impl EvalResult {
    pub fn new(eval: &Eval, results: &[(EvalAssertion, EvalAssertionResultEnum)]) -> EvalResult {
        let assertions: Vec<EvalAssertionResult> = results
            .iter()
            .map(|(assertion, result)| EvalAssertionResult {
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
            id: Uuid::new_v4(),
            eval_name: eval.name.clone(),
            passed,
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
pub struct EvalAssertionResult {
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

/// Represents a trace event or span from tracing-subscriber's JSON output
/// This struct is used for deserialization only
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceEvent {
    pub timestamp: String,
    pub level: String,
    #[serde(default)]
    pub target: String,
    #[serde(default)]
    pub fields: serde_json::Value,
    #[serde(default)]
    pub span: Option<SpanInfo>,
    #[serde(default)]
    pub spans: Vec<SpanInfo>,
}

/// Span metadata from tracing events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpanInfo {
    #[serde(default)]
    pub id: Option<u64>,
    pub name: String,
    #[serde(default)]
    pub fields: Option<serde_json::Value>,
}
