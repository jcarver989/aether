use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::store::TaskStoreError;
use super::tool_create::TaskSummary;
use super::types::{TaskId, TaskResult};
use super::TaskStore;

/// Input for the task_complete tool
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct TaskCompleteInput {
    /// Task ID to mark as completed
    pub id: String,

    /// Structured result (preferred for complex tasks)
    #[serde(default)]
    pub result: Option<TaskResult>,

    /// Simple text result (for backwards compatibility / trivial tasks)
    #[serde(default)]
    pub result_text: Option<String>,
}

/// Output for the task_complete tool
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct TaskCompleteOutput {
    /// Status of the operation
    pub status: String,

    /// The completed task
    pub task: TaskSummary,

    /// Human-readable message
    pub message: String,

    /// Tasks that are now ready to start (had this task as a dependency)
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub newly_ready: Vec<TaskSummary>,
}

/// Mark a task as completed with optional result/findings
pub fn execute_task_complete(
    input: TaskCompleteInput,
    store: &mut TaskStore,
) -> Result<TaskCompleteOutput, TaskStoreError> {
    let id = TaskId::from(input.id.as_str());

    // Get tasks that were blocked before completion
    let ready_before: Vec<String> = store.get_ready().iter().map(|t| t.id.to_string()).collect();

    // Complete the task with either structured or simple result
    let task = store.complete(&id, input.result, input.result_text.as_deref())?;

    // Find tasks that are now ready
    let newly_ready: Vec<TaskSummary> = store
        .get_ready()
        .iter()
        .filter(|t| !ready_before.contains(&t.id.to_string()))
        .map(|t| TaskSummary::from(*t))
        .collect();

    let message = if newly_ready.is_empty() {
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
    };

    Ok(TaskCompleteOutput {
        status: "success".to_string(),
        task: TaskSummary::from(&task),
        message,
        newly_ready,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coding::tools::tasks::TaskUpdate;
    use tempfile::TempDir;

    fn setup() -> (TempDir, TaskStore) {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path().join(".aether-tasks");
        let mut store = TaskStore::new(root);
        store.init().unwrap();
        (temp_dir, store)
    }

    #[test]
    fn test_complete_task() {
        let (_temp, mut store) = setup();

        let task = store.create_tree("Research task", None).unwrap();

        let output = execute_task_complete(
            TaskCompleteInput {
                id: task.id.to_string(),
                result: None,
                result_text: Some("Found great results!".to_string()),
            },
            &mut store,
        )
        .unwrap();

        assert_eq!(output.status, "success");
        assert_eq!(output.task.status, "completed");
        assert!(output.message.contains("Completed task"));
    }

    #[test]
    fn test_complete_unblocks_dependent() {
        let (_temp, mut store) = setup();

        let root = store.create_tree("Root", None).unwrap();
        let sub1 = store.add_subtask(&root.id, "Subtask 1").unwrap();
        let sub2 = store.add_subtask(&root.id, "Subtask 2").unwrap();

        // sub2 depends on sub1
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
        let output = execute_task_complete(
            TaskCompleteInput {
                id: sub1.id.to_string(),
                result: None,
                result_text: None,
            },
            &mut store,
        )
        .unwrap();

        // sub2 should now be ready
        assert_eq!(output.newly_ready.len(), 1);
        assert_eq!(output.newly_ready[0].id, sub2.id.to_string());
        assert!(output.message.contains("1 task is now ready"));
    }

    #[test]
    fn test_complete_unblocks_multiple() {
        let (_temp, mut store) = setup();

        let root = store.create_tree("Root", None).unwrap();
        let sub1 = store.add_subtask(&root.id, "Subtask 1").unwrap();
        let sub2 = store.add_subtask(&root.id, "Subtask 2").unwrap();
        let sub3 = store.add_subtask(&root.id, "Subtask 3").unwrap();

        // Both sub2 and sub3 depend on sub1
        store
            .update(
                &sub2.id,
                TaskUpdate {
                    deps: Some(vec![sub1.id.clone()]),
                    ..Default::default()
                },
            )
            .unwrap();
        store
            .update(
                &sub3.id,
                TaskUpdate {
                    deps: Some(vec![sub1.id.clone()]),
                    ..Default::default()
                },
            )
            .unwrap();

        // Complete sub1
        let output = execute_task_complete(
            TaskCompleteInput {
                id: sub1.id.to_string(),
                result: None,
                result_text: None,
            },
            &mut store,
        )
        .unwrap();

        // Both sub2 and sub3 should now be ready
        assert_eq!(output.newly_ready.len(), 2);
        assert!(output.message.contains("2 tasks are now ready"));
    }

    #[test]
    fn test_complete_nonexistent_task() {
        let (_temp, mut store) = setup();

        let result = execute_task_complete(
            TaskCompleteInput {
                id: "at-nonexistent".to_string(),
                result: None,
                result_text: None,
            },
            &mut store,
        );

        assert!(result.is_err());
    }

    #[test]
    fn test_complete_without_result() {
        let (_temp, mut store) = setup();

        let task = store.create_tree("Simple task", None).unwrap();

        let output = execute_task_complete(
            TaskCompleteInput {
                id: task.id.to_string(),
                result: None,
                result_text: None,
            },
            &mut store,
        )
        .unwrap();

        assert_eq!(output.status, "success");
        assert_eq!(output.task.status, "completed");
    }
}
