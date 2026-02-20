use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::common::TaskSummary;
use crate::task_store::{TaskStore, TaskStoreError};
use crate::types::{TaskId, TaskUpdate};
use mcp_coding::display_meta::ToolDisplayMeta;

/// Input for the `task_create` tool
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct TaskCreateInput {
    /// Short descriptive title for the task
    pub title: String,

    /// Detailed description (markdown)
    pub description: String,

    /// Parent task ID - if provided, creates a subtask
    #[serde(default)]
    pub parent_id: Option<String>,

    /// Agent or worker to assign the task to
    #[serde(default)]
    pub assignee: Option<String>,

    /// Task IDs that must complete before this task can start
    #[serde(default)]
    pub deps: Option<Vec<String>>,
}

/// Output for the `task_create` tool
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct TaskCreateOutput {
    /// Status of the operation
    pub status: String,

    /// The created task
    pub task: TaskSummary,

    /// Human-readable message
    pub message: String,

    /// Display metadata for human-friendly rendering
    #[serde(skip_serializing_if = "Option::is_none")]
    pub _meta: Option<serde_json::Value>,
}

/// Create a new task or subtask
pub fn execute_task_create(
    input: TaskCreateInput,
    store: &mut TaskStore,
) -> Result<TaskCreateOutput, TaskStoreError> {
    let task = if let Some(parent_id) = &input.parent_id {
        let parent = TaskId::from(parent_id.as_str());
        store.add_subtask(&parent, &input.title)?
    } else {
        store.create_tree(&input.title, Some(&input.description))?
    };

    let needs_update =
        input.assignee.is_some() || input.deps.is_some() || input.parent_id.is_some();

    let task = if needs_update {
        let update = TaskUpdate {
            description: if input.parent_id.is_some() {
                Some(input.description.clone())
            } else {
                None // Already set during create_tree
            },
            assignee: input.assignee.clone(),
            deps: input
                .deps
                .as_ref()
                .map(|d| d.iter().map(|s| TaskId::from(s.as_str())).collect()),
            ..Default::default()
        };
        store.update(&task.id, update)?
    } else {
        task
    };

    let task_type = if input.parent_id.is_some() {
        "subtask"
    } else {
        "task tree"
    };

    let display_meta = ToolDisplayMeta::todo_single(
        task.title.clone(),
        false,
        Some(format!("Creating {task_type}")),
    );

    Ok(TaskCreateOutput {
        status: "success".to_string(),
        message: format!("Created {} '{}' with ID {}", task_type, task.title, task.id),
        task: TaskSummary::from(&task),
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

    #[test]
    fn test_create_root_task() {
        let (_temp, mut store) = setup();

        let input = TaskCreateInput {
            title: "Research AI agents".to_string(),
            description: "Investigate multi-agent patterns".to_string(),
            parent_id: None,
            assignee: Some("orchestrator".to_string()),
            deps: None,
        };

        let output = execute_task_create(input, &mut store).unwrap();

        assert_eq!(output.status, "success");
        assert_eq!(output.task.title, "Research AI agents");
        assert_eq!(output.task.assignee, Some("orchestrator".to_string()));
        assert!(output.message.contains("task tree"));
    }

    #[test]
    fn test_create_subtask() {
        let (_temp, mut store) = setup();

        let root_input = TaskCreateInput {
            title: "Root task".to_string(),
            description: "Root task description".to_string(),
            parent_id: None,
            assignee: None,
            deps: None,
        };
        let root = execute_task_create(root_input, &mut store).unwrap();

        let sub_input = TaskCreateInput {
            title: "Subtask 1".to_string(),
            description: "Do something specific".to_string(),
            parent_id: Some(root.task.id.clone()),
            assignee: Some("worker-1".to_string()),
            deps: None,
        };

        let output = execute_task_create(sub_input, &mut store).unwrap();

        assert_eq!(output.status, "success");
        assert_eq!(output.task.title, "Subtask 1");
        assert_eq!(output.task.parent, Some(root.task.id));
        assert!(output.message.contains("subtask"));
    }

    #[test]
    fn test_create_with_deps() {
        let (_temp, mut store) = setup();

        let root = store.create_tree("Root", None).unwrap();
        let sub1 = store.add_subtask(&root.id, "Subtask 1").unwrap();

        let input = TaskCreateInput {
            title: "Subtask 2".to_string(),
            description: "Subtask with dependency".to_string(),
            parent_id: Some(root.id.to_string()),
            assignee: None,
            deps: Some(vec![sub1.id.to_string()]),
        };

        let output = execute_task_create(input, &mut store).unwrap();

        assert_eq!(output.task.deps, vec![sub1.id.to_string()]);
    }

    #[test]
    fn test_create_with_invalid_parent() {
        let (_temp, mut store) = setup();

        let input = TaskCreateInput {
            title: "Orphan".to_string(),
            description: "Orphan task".to_string(),
            parent_id: Some("at-nonexistent".to_string()),
            assignee: None,
            deps: None,
        };

        let result = execute_task_create(input, &mut store);
        assert!(result.is_err());
    }
}
