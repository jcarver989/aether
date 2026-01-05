use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::common::TaskSummary;
use crate::coding::display_meta::ToolDisplayMeta;
use crate::tasks::task_store::{TaskStore, TaskStoreError};
use crate::tasks::types::{Handoff, TaskId, TaskResult, TaskStatus, TaskUpdate};

/// Result data for completing a task.
///
/// When completing a task, provide this object with at minimum a `summary`.
/// The other fields capture context for downstream agents or future reference.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct TaskResultInput {
    /// Executive summary of work done (1-3 sentences). Required.
    pub summary: String,

    /// Key decisions made and why (e.g., "Chose X because Y")
    #[serde(default)]
    pub decisions: Vec<String>,

    /// Important facts discovered (e.g., "Found: error X in file Y")
    #[serde(default)]
    pub facts: Vec<String>,

    /// Suggested next steps for follow-up work
    #[serde(default)]
    pub next_steps: Vec<String>,

    /// Blockers or unresolved issues
    #[serde(default)]
    pub blockers: Vec<String>,

    /// Files examined (not modified - git tracks modifications)
    #[serde(default)]
    pub files_read: Vec<String>,

    /// External resources accessed with brief notes
    #[serde(default)]
    pub resources: Vec<String>,
}

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

    /// New status: pending, in_progress, or blocked.
    /// Do not set to "completed" directly; instead provide a `result` object.
    #[serde(default)]
    pub status: Option<TaskStatus>,

    /// New assignee (optional)
    #[serde(default)]
    pub assignee: Option<String>,

    /// New dependencies (optional, replaces existing)
    #[serde(default)]
    pub deps: Option<Vec<String>>,

    /// Completion result. Provide this to mark the task as completed.
    /// When provided, the task status is automatically set to "completed".
    #[serde(default)]
    pub result: Option<TaskResultInput>,
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

    /// Display metadata for human-friendly rendering
    #[serde(skip_serializing_if = "Option::is_none")]
    pub _meta: Option<serde_json::Value>,
}

/// Update an existing task's fields
pub fn execute_task_update(
    input: TaskUpdateInput,
    store: &mut TaskStore,
) -> Result<TaskUpdateOutput, TaskStoreError> {
    let id = TaskId::from(input.id.as_str());

    // Convert TaskResultInput to TaskResult if provided
    let result = input.result.map(|r| TaskResult {
        summary: r.summary,
        handoff: Handoff {
            decisions: r.decisions,
            facts: r.facts,
            next_steps: r.next_steps,
            blockers: r.blockers,
            files_read: r.files_read,
            resources: r.resources,
        },
    });

    // Validate task exists and check if re-completing
    if result.is_some()
        && let Some(task) = store.get(&id)
        && task.status == TaskStatus::Completed
    {
        return Err(TaskStoreError::ValidationError {
            message: "task is already completed; results cannot be overwritten".to_string(),
        });
    }

    // Determine effective status - if result provided, auto-set to completed
    let effective_status = match (&input.status, &result) {
        (Some(TaskStatus::Completed), None) => {
            return Err(TaskStoreError::ValidationError {
                message: "to complete a task, provide a 'result' object with a 'summary' field"
                    .to_string(),
            });
        }
        (Some(TaskStatus::Completed), Some(_)) => Some(TaskStatus::Completed),
        (Some(status), Some(_)) => {
            return Err(TaskStoreError::ValidationError {
                message: format!(
                    "cannot set status to '{}' when providing a result (result implies completed)",
                    status
                ),
            });
        }
        (None, Some(_)) => Some(TaskStatus::Completed), // Auto-complete when result provided
        (status, None) => *status,
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
    if result.is_some() {
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
        result,
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

    // Generate display metadata for the todo list
    let display_meta = ToolDisplayMeta::todo_single(
        task.title.clone(),
        effective_status == Some(TaskStatus::Completed),
        Some(if effective_status == Some(TaskStatus::Completed) {
            "Completing task".to_string()
        } else if effective_status == Some(TaskStatus::InProgress) {
            "Working on task".to_string()
        } else {
            "Updating task".to_string()
        }),
    );

    Ok(TaskUpdateOutput {
        status: "success".to_string(),
        task: TaskSummary::from(&task),
        message,
        changes,
        newly_ready,
        _meta: display_meta.into_meta(),
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

    /// Helper to create a default input with just the task ID
    fn input_for(id: &TaskId) -> TaskUpdateInput {
        TaskUpdateInput {
            id: id.to_string(),
            title: None,
            description: None,
            status: None,
            assignee: None,
            deps: None,
            result: None,
        }
    }

    /// Helper to create a result with just a summary
    fn result_with_summary(summary: &str) -> Option<TaskResultInput> {
        Some(TaskResultInput {
            summary: summary.to_string(),
            decisions: vec![],
            facts: vec![],
            next_steps: vec![],
            blockers: vec![],
            files_read: vec![],
            resources: vec![],
        })
    }

    #[test]
    fn test_update_title() {
        let (_temp, mut store) = setup();

        let task = store.create_tree("Original", None).unwrap();

        let input = TaskUpdateInput {
            title: Some("Updated title".to_string()),
            ..input_for(&task.id)
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
            status: Some(TaskStatus::InProgress),
            ..input_for(&task.id)
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
            title: Some("New title".to_string()),
            description: Some("New description".to_string()),
            status: Some(TaskStatus::InProgress),
            assignee: Some("worker-1".to_string()),
            ..input_for(&task.id)
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
            deps: Some(vec!["at-nonexistent".to_string()]),
            ..input_for(&task.id)
        };

        let result = execute_task_update(input, &mut store);
        assert!(result.is_err());
    }

    #[test]
    fn test_complete_task() {
        let (_temp, mut store) = setup();

        let task = store.create_tree("Task to complete", None).unwrap();

        let input = TaskUpdateInput {
            result: result_with_summary("Task done successfully"),
            ..input_for(&task.id)
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
            result: result_with_summary("Auto-completed"),
            ..input_for(&task.id)
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
            status: Some(TaskStatus::Completed),
            ..input_for(&task.id) // No result!
        };

        let result = execute_task_update(input, &mut store);
        assert!(result.is_err());
    }

    #[test]
    fn test_result_with_non_completed_status_fails() {
        let (_temp, mut store) = setup();

        let task = store.create_tree("Task", None).unwrap();

        let input = TaskUpdateInput {
            status: Some(TaskStatus::InProgress),
            result: result_with_summary("Conflicting"),
            ..input_for(&task.id)
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
            result: result_with_summary("Subtask 1 done"),
            ..input_for(&sub1.id)
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
            result: result_with_summary("First completion"),
            ..input_for(&task.id)
        };
        execute_task_update(input, &mut store).unwrap();

        // Try to complete again - should fail
        let input = TaskUpdateInput {
            result: result_with_summary("Second completion"),
            ..input_for(&task.id)
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

    #[test]
    fn test_complete_with_handoff_fields() {
        let (_temp, mut store) = setup();

        let task = store.create_tree("Research task", None).unwrap();

        let input = TaskUpdateInput {
            result: Some(TaskResultInput {
                summary: "Found 3 issues".to_string(),
                decisions: vec!["Use approach A".to_string()],
                facts: vec!["Issue in file X".to_string()],
                next_steps: vec!["Fix the issues".to_string()],
                blockers: vec![],
                files_read: vec![],
                resources: vec![],
            }),
            ..input_for(&task.id)
        };

        let output = execute_task_update(input, &mut store).unwrap();

        assert_eq!(output.task.status, "completed");
        assert!(output.changes.contains(&"result".to_string()));
    }
}
