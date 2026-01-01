use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::store::TaskStoreError;
use super::tool_create::TaskSummary;
use super::types::{TaskId, TaskStatus, TaskUpdate};
use super::TaskStore;

/// Input for the task_update tool
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct TaskUpdateInput {
    /// Task ID to update
    pub id: String,

    /// New title (optional)
    #[serde(default)]
    pub title: Option<String>,

    /// New description (optional, markdown)
    #[serde(default)]
    pub description: Option<String>,

    /// New status: pending, in_progress, or blocked
    /// Use task_complete to set status to completed
    #[serde(default)]
    pub status: Option<TaskStatusInput>,

    /// New assignee (optional)
    #[serde(default)]
    pub assignee: Option<String>,

    /// New dependencies (optional, replaces existing)
    #[serde(default)]
    pub deps: Option<Vec<String>>,
}

/// Status values that can be set via update (excludes completed)
#[derive(Debug, Clone, Copy, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatusInput {
    Pending,
    InProgress,
    Blocked,
}

impl From<TaskStatusInput> for TaskStatus {
    fn from(status: TaskStatusInput) -> Self {
        match status {
            TaskStatusInput::Pending => TaskStatus::Pending,
            TaskStatusInput::InProgress => TaskStatus::InProgress,
            TaskStatusInput::Blocked => TaskStatus::Blocked,
        }
    }
}

/// Output for the task_update tool
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct TaskUpdateOutput {
    /// Status of the operation
    pub status: String,

    /// The updated task
    pub task: TaskSummary,

    /// Human-readable message
    pub message: String,

    /// Fields that were changed
    pub changes: Vec<String>,
}

/// Update an existing task's fields
pub fn execute_task_update(
    input: TaskUpdateInput,
    store: &mut TaskStore,
) -> Result<TaskUpdateOutput, TaskStoreError> {
    let id = TaskId::from(input.id.as_str());

    // Track what's being changed
    let mut changes = Vec::new();

    if input.title.is_some() {
        changes.push("title".to_string());
    }
    if input.description.is_some() {
        changes.push("description".to_string());
    }
    if input.status.is_some() {
        changes.push("status".to_string());
    }
    if input.assignee.is_some() {
        changes.push("assignee".to_string());
    }
    if input.deps.is_some() {
        changes.push("deps".to_string());
    }

    let update = TaskUpdate {
        title: input.title,
        description: input.description,
        status: input.status.map(Into::into),
        assignee: input.assignee,
        deps: input
            .deps
            .map(|d| d.iter().map(|s| TaskId::from(s.as_str())).collect()),
    };

    let task = store.update(&id, update)?;

    let message = if changes.is_empty() {
        format!("No changes made to task {}", id)
    } else {
        format!("Updated {} on task {}", changes.join(", "), id)
    };

    Ok(TaskUpdateOutput {
        status: "success".to_string(),
        task: TaskSummary::from(&task),
        message,
        changes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup() -> (TempDir, TaskStore) {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path().join(".aether-tasks");
        let mut store = TaskStore::new(root);
        store.init().unwrap();
        (temp_dir, store)
    }

    #[test]
    fn test_update_title() {
        let (_temp, mut store) = setup();

        let task = store.create_tree("Original", None).unwrap();

        let input = TaskUpdateInput {
            id: task.id.to_string(),
            title: Some("Updated title".to_string()),
            description: None,
            status: None,
            assignee: None,
            deps: None,
        };

        let output = execute_task_update(input, &mut store).unwrap();

        assert_eq!(output.status, "success");
        assert_eq!(output.task.title, "Updated title");
        assert_eq!(output.changes, vec!["title"]);
    }

    #[test]
    fn test_update_status() {
        let (_temp, mut store) = setup();

        let task = store.create_tree("Task", None).unwrap();

        let input = TaskUpdateInput {
            id: task.id.to_string(),
            title: None,
            description: None,
            status: Some(TaskStatusInput::InProgress),
            assignee: None,
            deps: None,
        };

        let output = execute_task_update(input, &mut store).unwrap();

        assert_eq!(output.task.status, "in_progress");
        assert_eq!(output.changes, vec!["status"]);
    }

    #[test]
    fn test_update_multiple_fields() {
        let (_temp, mut store) = setup();

        let task = store.create_tree("Task", None).unwrap();

        let input = TaskUpdateInput {
            id: task.id.to_string(),
            title: Some("New title".to_string()),
            description: Some("New description".to_string()),
            status: Some(TaskStatusInput::InProgress),
            assignee: Some("worker-1".to_string()),
            deps: None,
        };

        let output = execute_task_update(input, &mut store).unwrap();

        assert_eq!(output.changes.len(), 4);
        assert!(output.changes.contains(&"title".to_string()));
        assert!(output.changes.contains(&"description".to_string()));
        assert!(output.changes.contains(&"status".to_string()));
        assert!(output.changes.contains(&"assignee".to_string()));
    }

    #[test]
    fn test_update_nonexistent_task() {
        let (_temp, mut store) = setup();

        let input = TaskUpdateInput {
            id: "at-nonexistent".to_string(),
            title: Some("New title".to_string()),
            description: None,
            status: None,
            assignee: None,
            deps: None,
        };

        let result = execute_task_update(input, &mut store);
        assert!(result.is_err());
    }

    #[test]
    fn test_update_with_invalid_deps() {
        let (_temp, mut store) = setup();

        let task = store.create_tree("Task", None).unwrap();

        let input = TaskUpdateInput {
            id: task.id.to_string(),
            title: None,
            description: None,
            status: None,
            assignee: None,
            deps: Some(vec!["at-nonexistent".to_string()]),
        };

        let result = execute_task_update(input, &mut store);
        assert!(result.is_err());
    }
}
