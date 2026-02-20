use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    Eval, EvalAssertion, evals::assertion::EvalAssertionResult as EvalAssertionResultEnum,
};

/// Result of an evaluation in various states
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "lowercase")]
pub enum EvalResult {
    /// Eval has started but not yet running
    Started { id: Uuid, eval_name: String },
    /// Eval is currently running
    Running { id: Uuid, eval_name: String },
    /// Eval has completed with results
    Completed {
        id: Uuid,
        eval_name: String,
        passed: bool,
        assertions: Vec<EvalAssertionResult>,
        #[serde(skip_serializing_if = "Option::is_none")]
        agent_diff: Option<GitDiff>,
        #[serde(skip_serializing_if = "Option::is_none")]
        reference_diff: Option<GitDiff>,
    },
}

impl EvalResult {
    /// Create a new `EvalResult` in "Started" state
    pub fn started(eval: &Eval, eval_id: Uuid) -> Self {
        EvalResult::Started {
            id: eval_id,
            eval_name: eval.name.clone(),
        }
    }

    /// Create a completed `EvalResult` with assertion results
    pub fn completed(
        eval: &Eval,
        eval_id: Uuid,
        results: &[(EvalAssertion, EvalAssertionResultEnum)],
    ) -> EvalResult {
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

        EvalResult::Completed {
            id: eval_id,
            eval_name: eval.name.clone(),
            passed,
            assertions,
            agent_diff: None,
            reference_diff: None,
        }
    }

    /// Get the eval ID regardless of state
    pub fn id(&self) -> Uuid {
        match self {
            EvalResult::Started { id, .. }
            | EvalResult::Running { id, .. }
            | EvalResult::Completed { id, .. } => *id,
        }
    }

    /// Get the eval name regardless of state
    pub fn eval_name(&self) -> &str {
        match self {
            EvalResult::Started { eval_name, .. }
            | EvalResult::Running { eval_name, .. }
            | EvalResult::Completed { eval_name, .. } => eval_name,
        }
    }

    /// Check if this eval has completed
    pub fn is_completed(&self) -> bool {
        matches!(self, EvalResult::Completed { .. })
    }

    /// Get whether the eval passed (only for completed evals)
    pub fn passed(&self) -> Option<bool> {
        match self {
            EvalResult::Completed { passed, .. } => Some(*passed),
            _ => None,
        }
    }

    /// Update the agent diff (only for completed evals)
    pub fn set_agent_diff(&mut self, diff: GitDiff) {
        if let EvalResult::Completed { agent_diff, .. } = self {
            *agent_diff = Some(diff);
        }
    }

    /// Update the reference diff (only for completed evals)
    pub fn set_reference_diff(&mut self, diff: GitDiff) {
        if let EvalResult::Completed { reference_diff, .. } = self {
            *reference_diff = Some(diff);
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
    /// Captures all other fields (like `eval_name`, `eval_id`, etc.) at the span level
    #[serde(flatten)]
    pub extra: serde_json::Value,
}
