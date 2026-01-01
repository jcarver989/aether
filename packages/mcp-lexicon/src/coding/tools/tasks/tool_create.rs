use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::store::TaskStoreError;
use super::types::{Task, TaskId, TaskUpdate};
use super::TaskStore;

/// Input for the task_create tool
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct TaskCreateInput {
    /// Short descriptive title for the task
    pub title: String,

    /// Optional detailed description (markdown)
    #[serde(default)]
    pub description: Option<String>,

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

/// Output for the task_create tool
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct TaskCreateOutput {
    /// Status of the operation
    pub status: String,

    /// The created task
    pub task: TaskSummary,

    /// Human-readable message
    pub message: String,
}

/// Summary of a task for tool output
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

/// Create a new task or subtask
pub fn execute_task_create(
    input: TaskCreateInput,
    store: &mut TaskStore,
) -> Result<TaskCreateOutput, TaskStoreError> {
    let task = if let Some(parent_id) = &input.parent_id {
        // Create subtask
        let parent = TaskId::from(parent_id.as_str());
        store.add_subtask(&parent, &input.title)?
    } else {
        // Create root task
        store.create_tree(&input.title, input.description.as_deref())?
    };

    // Apply optional updates (assignee, deps, description for subtasks)
    let needs_update = input.assignee.is_some()
        || input.deps.is_some()
        || (input.parent_id.is_some() && input.description.is_some());

    let task = if needs_update {
        let update = TaskUpdate {
            description: if input.parent_id.is_some() {
                input.description.clone()
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

    Ok(TaskCreateOutput {
        status: "success".to_string(),
        message: format!("Created {} '{}' with ID {}", task_type, task.title, task.id),
        task: TaskSummary::from(&task),
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
            description: Some("Investigate multi-agent patterns".to_string()),
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

        // Create root first
        let root_input = TaskCreateInput {
            title: "Root task".to_string(),
            description: None,
            parent_id: None,
            assignee: None,
            deps: None,
        };
        let root = execute_task_create(root_input, &mut store).unwrap();

        // Create subtask
        let sub_input = TaskCreateInput {
            title: "Subtask 1".to_string(),
            description: Some("Do something specific".to_string()),
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

        // Create root and first subtask
        let root = store.create_tree("Root", None).unwrap();
        let sub1 = store.add_subtask(&root.id, "Subtask 1").unwrap();

        // Create subtask with dependency
        let input = TaskCreateInput {
            title: "Subtask 2".to_string(),
            description: None,
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
            description: None,
            parent_id: Some("at-nonexistent".to_string()),
            assignee: None,
            deps: None,
        };

        let result = execute_task_create(input, &mut store);
        assert!(result.is_err());
    }
}
