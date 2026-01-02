//! Common output types shared across task tools.

use schemars::JsonSchema;
use serde::Serialize;

use crate::tasks::types::Task;

/// Summary of a task for tool output (compact form)
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct TaskSummary {
    pub id: String,
    pub title: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assignee: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub deps: Vec<String>,
}

impl From<&Task> for TaskSummary {
    fn from(task: &Task) -> Self {
        Self {
            id: task.id.to_string(),
            title: task.title.clone(),
            status: task.status.to_string(),
            parent: task.parent.as_ref().map(|p| p.to_string()),
            assignee: task.assignee.clone(),
            deps: task.deps.iter().map(|d| d.to_string()).collect(),
        }
    }
}

/// Full task details for tool output (includes result and timestamps)
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct TaskDetail {
    pub id: String,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assignee: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub deps: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<TaskResultDetail>,
    pub created_at: String,
    pub updated_at: String,
}

/// Task result details for tool output
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct TaskResultDetail {
    pub summary: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub handoff: Option<HandoffDetail>,
}

/// Handoff context details for tool output
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct HandoffDetail {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub decisions: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub facts: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub next_steps: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub blockers: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub files_read: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub resources: Vec<String>,
}

impl From<&Task> for TaskDetail {
    fn from(task: &Task) -> Self {
        Self {
            id: task.id.to_string(),
            title: task.title.clone(),
            description: task.description.clone(),
            status: task.status.to_string(),
            assignee: task.assignee.clone(),
            parent: task.parent.as_ref().map(|p| p.to_string()),
            deps: task.deps.iter().map(|d| d.to_string()).collect(),
            result: task.result.as_ref().map(TaskResultDetail::from),
            created_at: task.created_at.to_rfc3339(),
            updated_at: task.updated_at.to_rfc3339(),
        }
    }
}

impl From<&crate::tasks::types::TaskResult> for TaskResultDetail {
    fn from(result: &crate::tasks::types::TaskResult) -> Self {
        Self {
            summary: result.summary.clone(),
            handoff: if result.handoff.is_empty() {
                None
            } else {
                Some(HandoffDetail::from(&result.handoff))
            },
        }
    }
}

impl From<&crate::tasks::types::Handoff> for HandoffDetail {
    fn from(handoff: &crate::tasks::types::Handoff) -> Self {
        Self {
            decisions: handoff.decisions.clone(),
            facts: handoff.facts.clone(),
            next_steps: handoff.next_steps.clone(),
            blockers: handoff.blockers.clone(),
            files_read: handoff.files_read.clone(),
            resources: handoff.resources.clone(),
        }
    }
}
