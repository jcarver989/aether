use schemars::JsonSchema;
use serde::Serialize;

use crate::tasks::types::Task;

/// Summary of a task for tool output (compact form for list views)
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
