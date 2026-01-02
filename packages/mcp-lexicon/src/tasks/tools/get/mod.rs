use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::common::TaskDetail;
use crate::tasks::task_store::{TaskStore, TaskStoreError};

/// Input for the task_get tool
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct TaskGetInput {
    /// Task ID to retrieve (e.g., "at-a1b2c3d4" or "at-a1b2c3d4.1")
    pub id: String,
}

/// Output for the task_get tool
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct TaskGetOutput {
    /// Status of the operation
    pub status: String,

    /// The full task object
    pub task: TaskDetail,

    /// Human-readable message
    pub message: String,
}

/// Get a task by ID
pub fn execute_task_get(
    input: TaskGetInput,
    store: &TaskStore,
) -> Result<TaskGetOutput, TaskStoreError> {
    let task_id = crate::tasks::types::TaskId::from(input.id.as_str());

    let task = store
        .get(&task_id)
        .ok_or(TaskStoreError::NotFound { id: input.id })?;

    Ok(TaskGetOutput {
        status: "success".to_string(),
        message: format!("Retrieved task '{}'", task.title),
        task: TaskDetail::from(task),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tasks::Handoff;
    use crate::tasks::types::{TaskResult, TaskStatus, TaskUpdate};
    use tempfile::TempDir;

    fn setup() -> (TempDir, TaskStore) {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path().join(".aether-tasks");
        let mut store = TaskStore::new(root);
        store.init().unwrap();
        (temp_dir, store)
    }

    #[test]
    fn test_get_root_task() {
        let (_temp, mut store) = setup();

        let created = store
            .create_tree("Research topic", Some("Detailed description"))
            .unwrap();

        let output = execute_task_get(
            TaskGetInput {
                id: created.id.to_string(),
            },
            &store,
        )
        .unwrap();

        assert_eq!(output.status, "success");
        assert_eq!(output.task.id, created.id.to_string());
        assert_eq!(output.task.title, "Research topic");
        assert_eq!(
            output.task.description,
            Some("Detailed description".to_string())
        );
        assert_eq!(output.task.status, "pending");
        assert!(output.message.contains("Retrieved task"));
    }

    #[test]
    fn test_get_subtask() {
        let (_temp, mut store) = setup();

        let root = store.create_tree("Root", None).unwrap();
        let subtask = store.add_subtask(&root.id, "Subtask 1").unwrap();

        let output = execute_task_get(
            TaskGetInput {
                id: subtask.id.to_string(),
            },
            &store,
        )
        .unwrap();

        assert_eq!(output.task.id, subtask.id.to_string());
        assert_eq!(output.task.title, "Subtask 1");
        assert_eq!(output.task.parent, Some(root.id.to_string()));
    }

    #[test]
    fn test_get_task_with_result() {
        let (_temp, mut store) = setup();

        let task = store.create_tree("Task to complete", None).unwrap();

        store
            .update(
                &task.id,
                TaskUpdate {
                    status: Some(TaskStatus::Completed),
                    result: Some(TaskResult {
                        summary: "Found the answer".to_string(),
                        handoff: Handoff {
                            decisions: vec!["Chose option A".to_string()],
                            facts: vec!["Discovered X".to_string()],
                            next_steps: vec!["Do Y next".to_string()],
                            blockers: vec![],
                            files_read: vec!["src/main.rs".to_string()],
                            resources: vec![],
                        },
                    }),
                    ..Default::default()
                },
            )
            .unwrap();

        let output = execute_task_get(
            TaskGetInput {
                id: task.id.to_string(),
            },
            &store,
        )
        .unwrap();

        assert_eq!(output.task.status, "completed");

        let result = output.task.result.unwrap();
        assert_eq!(result.summary, "Found the answer");

        let handoff = result.handoff.unwrap();
        assert_eq!(handoff.decisions, vec!["Chose option A"]);
        assert_eq!(handoff.facts, vec!["Discovered X"]);
        assert_eq!(handoff.next_steps, vec!["Do Y next"]);
        assert_eq!(handoff.files_read, vec!["src/main.rs"]);
    }

    #[test]
    fn test_get_task_with_deps() {
        let (_temp, mut store) = setup();

        let root = store.create_tree("Root", None).unwrap();
        let sub1 = store.add_subtask(&root.id, "Subtask 1").unwrap();
        let sub2 = store.add_subtask(&root.id, "Subtask 2").unwrap();

        store
            .update(
                &sub2.id,
                TaskUpdate {
                    deps: Some(vec![sub1.id.clone()]),
                    ..Default::default()
                },
            )
            .unwrap();

        let output = execute_task_get(
            TaskGetInput {
                id: sub2.id.to_string(),
            },
            &store,
        )
        .unwrap();

        assert_eq!(output.task.deps, vec![sub1.id.to_string()]);
    }

    #[test]
    fn test_get_nonexistent_task() {
        let (_temp, store) = setup();

        let result = execute_task_get(
            TaskGetInput {
                id: "at-nonexistent".to_string(),
            },
            &store,
        );

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            TaskStoreError::NotFound { .. }
        ));
    }
}
