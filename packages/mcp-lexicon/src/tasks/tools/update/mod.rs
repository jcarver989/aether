use super::create::TaskSummary;
use crate::tasks::task_store::{TaskStore, TaskStoreError};
use crate::tasks::types::{TaskId, TaskResult, TaskStatus, TaskUpdate};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

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

    /// New status: pending, in_progress, blocked, or completed
    /// When setting to completed, you must also provide a result
    #[serde(default)]
    pub status: Option<TaskStatus>,

    /// New assignee (optional)
    #[serde(default)]
    pub assignee: Option<String>,

    /// New dependencies (optional, replaces existing)
    #[serde(default)]
    pub deps: Option<Vec<String>>,

    /// Completion result - required when status is "completed"
    /// If provided without status, status will be set to "completed" automatically
    #[serde(default)]
    pub result: Option<TaskResult>,
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

    /// Tasks that are now ready to start (when completing a task that others depended on)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub newly_ready: Vec<TaskSummary>,
}

/// Update an existing task's fields
pub fn execute_task_update(
    input: TaskUpdateInput,
    store: &mut TaskStore,
) -> Result<TaskUpdateOutput, TaskStoreError> {
    let id = TaskId::from(input.id.as_str());

    // Validate task exists and check if re-completing
    if input.result.is_some()
        && let Some(task) = store.get(&id)
        && task.status == TaskStatus::Completed
    {
        return Err(TaskStoreError::ValidationError {
            message: "task is already completed; results cannot be overwritten".to_string(),
        });
    }

    // Determine effective status - if result provided, auto-set to completed
    let effective_status = match (&input.status, &input.result) {
        (Some(TaskStatus::Completed), None) => {
            return Err(TaskStoreError::ValidationError {
                message: "result is required when setting status to completed".to_string(),
            });
        }
        (Some(status), _) if *status != TaskStatus::Completed && input.result.is_some() => {
            return Err(TaskStoreError::ValidationError {
                message: format!(
                    "cannot provide result when setting status to {} (result implies completed)",
                    status
                ),
            });
        }
        (None, Some(_)) => Some(TaskStatus::Completed), // Auto-complete when result provided
        (status, _) => *status,
    };

    let mut changes = Vec::new();

    if input.title.is_some() {
        changes.push("title".to_string());
    }
    if input.description.is_some() {
        changes.push("description".to_string());
    }
    if effective_status.is_some() {
        changes.push("status".to_string());
    }
    if input.assignee.is_some() {
        changes.push("assignee".to_string());
    }
    if input.deps.is_some() {
        changes.push("deps".to_string());
    }
    if input.result.is_some() {
        changes.push("result".to_string());
    }

    // Track ready tasks before the update (for detecting newly ready tasks)
    let ready_before: Vec<String> = if effective_status == Some(TaskStatus::Completed) {
        store.get_ready().iter().map(|t| t.id.to_string()).collect()
    } else {
        vec![]
    };

    // Apply the update
    let update = TaskUpdate {
        title: input.title,
        description: input.description,
        status: effective_status,
        assignee: input.assignee,
        deps: input
            .deps
            .map(|d| d.iter().map(|s| TaskId::from(s.as_str())).collect()),
        result: input.result,
    };

    let task = store.update(&id, update)?;

    // Find newly ready tasks (only relevant when completing)
    let newly_ready: Vec<TaskSummary> = if effective_status == Some(TaskStatus::Completed) {
        store
            .get_ready()
            .iter()
            .filter(|t| !ready_before.contains(&t.id.to_string()))
            .map(|t| TaskSummary::from(*t))
            .collect()
    } else {
        vec![]
    };

    let message = if changes.is_empty() {
        format!("No changes made to task {}", id)
    } else if effective_status == Some(TaskStatus::Completed) {
        if newly_ready.is_empty() {
            format!("Completed task '{}'", task.title)
        } else if newly_ready.len() == 1 {
            format!(
                "Completed task '{}'. 1 task is now ready: {}",
                task.title, newly_ready[0].title
            )
        } else {
            format!(
                "Completed task '{}'. {} tasks are now ready",
                task.title,
                newly_ready.len()
            )
        }
    } else {
        format!("Updated {} on task {}", changes.join(", "), id)
    };

    Ok(TaskUpdateOutput {
        status: "success".to_string(),
        task: TaskSummary::from(&task),
        message,
        changes,
        newly_ready,
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
            result: None,
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
            status: Some(TaskStatus::InProgress),
            assignee: None,
            deps: None,
            result: None,
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
            status: Some(TaskStatus::InProgress),
            assignee: Some("worker-1".to_string()),
            deps: None,
            result: None,
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
            result: None,
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
            result: None,
        };

        let result = execute_task_update(input, &mut store);
        assert!(result.is_err());
    }

    #[test]
    fn test_complete_task() {
        let (_temp, mut store) = setup();

        let task = store.create_tree("Task to complete", None).unwrap();

        let input = TaskUpdateInput {
            id: task.id.to_string(),
            title: None,
            description: None,
            status: Some(TaskStatus::Completed),
            assignee: None,
            deps: None,
            result: Some(TaskResult::new("Task done successfully")),
        };

        let output = execute_task_update(input, &mut store).unwrap();

        assert_eq!(output.task.status, "completed");
        assert!(output.message.contains("Completed task"));
    }

    #[test]
    fn test_complete_with_result_auto_sets_status() {
        let (_temp, mut store) = setup();

        let task = store.create_tree("Task", None).unwrap();

        // Providing result without status should auto-complete
        let input = TaskUpdateInput {
            id: task.id.to_string(),
            title: None,
            description: None,
            status: None,
            assignee: None,
            deps: None,
            result: Some(TaskResult::new("Auto-completed")),
        };

        let output = execute_task_update(input, &mut store).unwrap();

        assert_eq!(output.task.status, "completed");
        assert!(output.changes.contains(&"status".to_string()));
        assert!(output.changes.contains(&"result".to_string()));
    }

    #[test]
    fn test_complete_without_result_fails() {
        let (_temp, mut store) = setup();

        let task = store.create_tree("Task", None).unwrap();

        let input = TaskUpdateInput {
            id: task.id.to_string(),
            title: None,
            description: None,
            status: Some(TaskStatus::Completed),
            assignee: None,
            deps: None,
            result: None, // Missing result!
        };

        let result = execute_task_update(input, &mut store);
        assert!(result.is_err());
    }

    #[test]
    fn test_result_with_non_completed_status_fails() {
        let (_temp, mut store) = setup();

        let task = store.create_tree("Task", None).unwrap();

        let input = TaskUpdateInput {
            id: task.id.to_string(),
            title: None,
            description: None,
            status: Some(TaskStatus::InProgress),
            assignee: None,
            deps: None,
            result: Some(TaskResult::new("Conflicting")),
        };

        let result = execute_task_update(input, &mut store);
        assert!(result.is_err());
    }

    #[test]
    fn test_complete_unblocks_dependent() {
        let (_temp, mut store) = setup();

        let root = store.create_tree("Root", None).unwrap();
        let sub1 = store.add_subtask(&root.id, "Subtask 1").unwrap();
        let sub2 = store.add_subtask(&root.id, "Subtask 2").unwrap();

        // Make sub2 depend on sub1
        store
            .update(
                &sub2.id,
                TaskUpdate {
                    deps: Some(vec![sub1.id.clone()]),
                    ..Default::default()
                },
            )
            .unwrap();

        // Complete sub1
        let input = TaskUpdateInput {
            id: sub1.id.to_string(),
            title: None,
            description: None,
            status: Some(TaskStatus::Completed),
            assignee: None,
            deps: None,
            result: Some(TaskResult::new("Subtask 1 done")),
        };

        let output = execute_task_update(input, &mut store).unwrap();

        assert_eq!(output.newly_ready.len(), 1);
        assert_eq!(output.newly_ready[0].id, sub2.id.to_string());
        assert!(output.message.contains("1 task is now ready"));
    }

    #[test]
    fn test_cannot_recomplete_task() {
        let (_temp, mut store) = setup();

        let task = store.create_tree("Task", None).unwrap();

        // Complete the task
        let input = TaskUpdateInput {
            id: task.id.to_string(),
            title: None,
            description: None,
            status: Some(TaskStatus::Completed),
            assignee: None,
            deps: None,
            result: Some(TaskResult::new("First completion")),
        };
        execute_task_update(input, &mut store).unwrap();

        // Try to complete again - should fail
        let input = TaskUpdateInput {
            id: task.id.to_string(),
            title: None,
            description: None,
            status: None,
            assignee: None,
            deps: None,
            result: Some(TaskResult::new("Second completion")),
        };
        let result = execute_task_update(input, &mut store);

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("already completed")
        );
    }
}
