use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::hash::Hash;

/// Structured task completion result.
///
/// Captures information that survives context compression for handoff to downstream agents.
/// File modifications are tracked by git (`git diff --name-only`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct TaskResult {
    /// Executive summary (1-3 sentences)
    pub summary: String,

    /// Context for downstream agents starting with empty context window
    #[serde(default, skip_serializing_if = "Handoff::is_empty")]
    pub handoff: Handoff,
}

impl TaskResult {
    /// Create a minimal result with just a summary
    pub fn new(summary: impl Into<String>) -> Self {
        Self {
            summary: summary.into(),
            handoff: Handoff::default(),
        }
    }
}

/// Everything needed to hand off work to a fresh agent.
///
/// All fields are flat strings - agents write natural language like:
/// - decisions: "Chose X because Y"
/// - facts: "Found: error X in file Y"
/// - resources: "https://example.com - brief description"
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct Handoff {
    /// Key decisions made (what was decided and why)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub decisions: Vec<String>,

    /// Important facts discovered (errors, patterns, constraints)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub facts: Vec<String>,

    /// Suggested next steps
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub next_steps: Vec<String>,

    /// Blockers or unresolved issues
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub blockers: Vec<String>,

    /// Files examined (not modified - git tracks modifications)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub files_read: Vec<String>,

    /// External resources accessed with brief notes
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub resources: Vec<String>,
}

impl Handoff {
    pub fn is_empty(&self) -> bool {
        self.decisions.is_empty()
            && self.facts.is_empty()
            && self.next_steps.is_empty()
            && self.blockers.is_empty()
            && self.files_read.is_empty()
            && self.resources.is_empty()
    }
}

/// Task identifier with hierarchical support.
/// - Root tasks: `at-{hash8}` (e.g., `at-a1b2c3d4`)
/// - Subtasks: `at-{hash8}.{n}` (e.g., `at-a1b2c3d4.1`, `at-a1b2c3d4.2`)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(transparent)]
pub struct TaskId(String);

impl TaskId {
    /// Create a new root task ID from a hash
    pub fn new_root(hash: &str) -> Self {
        Self(format!("at-{hash}"))
    }

    /// Create a subtask ID from a parent and index
    pub fn new_subtask(parent: &TaskId, index: usize) -> Self {
        Self(format!("{}.{}", parent.0, index))
    }

    /// Get the root task ID (strips subtask suffix if present)
    pub fn root(&self) -> TaskId {
        if let Some(dot_pos) = self.0.find('.') {
            TaskId(self.0[..dot_pos].to_string())
        } else {
            self.clone()
        }
    }

    /// Check if this is a root task (no parent)
    pub fn is_root(&self) -> bool {
        !self.0.contains('.')
    }

    /// Get the parent task ID if this is a subtask
    pub fn parent(&self) -> Option<TaskId> {
        self.0
            .rfind('.')
            .map(|dot_pos| TaskId(self.0[..dot_pos].to_string()))
    }

    /// Get the raw string representation
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Generate the filename for this task tree (based on root ID)
    pub fn filename(&self) -> String {
        format!("{}.jsonl", self.root().0)
    }
}

impl fmt::Display for TaskId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for TaskId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for TaskId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

/// Task status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Pending,
    InProgress,
    Completed,
    Blocked,
}

impl fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TaskStatus::Pending => write!(f, "pending"),
            TaskStatus::InProgress => write!(f, "in_progress"),
            TaskStatus::Completed => write!(f, "completed"),
            TaskStatus::Blocked => write!(f, "blocked"),
        }
    }
}

/// A task for tracking work in deep research workflows
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct Task {
    /// Unique task identifier
    pub id: TaskId,

    /// Short descriptive title
    pub title: String,

    /// Optional detailed description (markdown)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Current status
    pub status: TaskStatus,

    /// Agent or worker assigned to this task
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assignee: Option<String>,

    /// Parent task ID (for subtasks)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent: Option<TaskId>,

    /// Dependencies - task IDs that must complete before this starts
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub deps: Vec<TaskId>,

    /// Structured findings/output when completed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<TaskResult>,

    /// When the task was created
    pub created_at: DateTime<Utc>,

    /// When the task was last updated
    pub updated_at: DateTime<Utc>,
}

impl Task {
    /// Create a new root task
    pub fn new_root(id: TaskId, title: String) -> Self {
        let now = Utc::now();
        Self {
            id,
            title,
            description: None,
            status: TaskStatus::Pending,
            assignee: None,
            parent: None,
            deps: Vec::new(),
            result: None,
            created_at: now,
            updated_at: now,
        }
    }

    /// Create a new subtask
    pub fn new_subtask(id: TaskId, parent: TaskId, title: String) -> Self {
        let now = Utc::now();
        Self {
            id,
            title,
            description: None,
            status: TaskStatus::Pending,
            assignee: None,
            parent: Some(parent),
            deps: Vec::new(),
            result: None,
            created_at: now,
            updated_at: now,
        }
    }

    /// Check if the task can be started (all dependencies completed)
    pub fn is_ready(&self, completed_tasks: &[&TaskId]) -> bool {
        self.status == TaskStatus::Pending
            && self.deps.iter().all(|dep| completed_tasks.contains(&dep))
    }

    /// Update the task's updated_at timestamp
    pub fn touch(&mut self) {
        self.updated_at = Utc::now();
    }
}

/// Updates that can be applied to a task
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct TaskUpdate {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<TaskStatus>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub assignee: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub deps: Option<Vec<TaskId>>,

    /// Completion result - when provided, task status is set to completed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<TaskResult>,
}

impl TaskUpdate {
    /// Apply updates to a task, consuming the update
    pub fn apply_to(self, task: &mut Task) {
        if let Some(title) = self.title {
            task.title = title;
        }
        if let Some(description) = self.description {
            task.description = Some(description);
        }
        if let Some(status) = self.status {
            task.status = status;
        }
        if let Some(assignee) = self.assignee {
            task.assignee = Some(assignee);
        }
        if let Some(deps) = self.deps {
            task.deps = deps;
        }
        if let Some(result) = self.result {
            task.result = Some(result);
        }
        task.touch();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_id_root() {
        let id = TaskId::new_root("a1b2c3d4");
        assert_eq!(id.as_str(), "at-a1b2c3d4");
        assert!(id.is_root());
        assert_eq!(id.parent(), None);
        assert_eq!(id.filename(), "at-a1b2c3d4.jsonl");
    }

    #[test]
    fn test_task_id_subtask() {
        let root = TaskId::new_root("a1b2c3d4");
        let subtask = TaskId::new_subtask(&root, 1);
        assert_eq!(subtask.as_str(), "at-a1b2c3d4.1");
        assert!(!subtask.is_root());
        assert_eq!(subtask.parent(), Some(root.clone()));
        assert_eq!(subtask.root(), root);
        assert_eq!(subtask.filename(), "at-a1b2c3d4.jsonl");
    }

    #[test]
    fn test_task_id_nested_subtask() {
        let root = TaskId::new_root("a1b2c3d4");
        let subtask1 = TaskId::new_subtask(&root, 1);
        let subtask2 = TaskId::new_subtask(&subtask1, 2);
        assert_eq!(subtask2.as_str(), "at-a1b2c3d4.1.2");
        assert_eq!(subtask2.parent(), Some(subtask1));
        assert_eq!(subtask2.root(), root);
    }

    #[test]
    fn test_task_is_ready() {
        let id1 = TaskId::new_root("task1");
        let id2 = TaskId::new_root("task2");
        let id3 = TaskId::new_root("task3");

        let mut task = Task::new_root(id3.clone(), "Test Task".to_string());
        task.deps = vec![id1.clone(), id2.clone()];

        assert!(!task.is_ready(&[]));

        assert!(!task.is_ready(&[&id1]));

        assert!(task.is_ready(&[&id1, &id2]));

        task.status = TaskStatus::InProgress;
        assert!(!task.is_ready(&[&id1, &id2]));
    }

    #[test]
    fn test_task_update_apply() {
        let id = TaskId::new_root("test");
        let mut task = Task::new_root(id, "Original".to_string());

        let update = TaskUpdate {
            title: Some("Updated".to_string()),
            status: Some(TaskStatus::InProgress),
            assignee: Some("worker-1".to_string()),
            ..Default::default()
        };

        update.apply_to(&mut task);

        assert_eq!(task.title, "Updated");
        assert_eq!(task.status, TaskStatus::InProgress);
        assert_eq!(task.assignee, Some("worker-1".to_string()));
    }

    #[test]
    fn test_task_serialization() {
        let id = TaskId::new_root("a1b2c3d4");
        let task = Task::new_root(id, "Test Task".to_string());

        let json = serde_json::to_string(&task).unwrap();
        assert!(json.contains("\"id\":\"at-a1b2c3d4\""));
        assert!(json.contains("\"status\":\"pending\""));

        let deserialized: Task = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id.as_str(), "at-a1b2c3d4");
        assert_eq!(deserialized.status, TaskStatus::Pending);
    }

    #[test]
    fn test_task_result_full_serialization() {
        let result = TaskResult {
            summary: "Identified 5 API endpoints using deprecated auth".to_string(),
            handoff: Handoff {
                decisions: vec![
                    "Defer JWT migration until after v2.0 - breaking change requires SDK updates"
                        .to_string(),
                ],
                facts: vec![
                    "All endpoints use validate_session() for auth (src/api/*.rs)".to_string(),
                    "Session tokens expire after 1 hour with no refresh".to_string(),
                ],
                next_steps: vec!["Create migration guide".to_string()],
                blockers: vec!["Need product decision on migration timeline".to_string()],
                files_read: vec!["src/api/auth.rs".to_string()],
                resources: vec!["https://docs.rs/jsonwebtoken - supports RS256/ES256".to_string()],
            },
        };

        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"summary\":\"Identified 5 API endpoints"));
        assert!(json.contains("\"handoff\":{"));
        assert!(json.contains("\"decisions\":["));
        assert!(json.contains("\"facts\":["));
        assert!(json.contains("\"next_steps\":["));
        assert!(json.contains("\"blockers\":["));
        assert!(json.contains("\"files_read\":["));
        assert!(json.contains("\"resources\":["));

        let deserialized: TaskResult = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.summary, result.summary);
        assert_eq!(deserialized.handoff.decisions.len(), 1);
        assert_eq!(deserialized.handoff.facts.len(), 2);
        assert_eq!(deserialized.handoff.next_steps.len(), 1);
    }

    #[test]
    fn test_task_result_minimal_serialization() {
        let result = TaskResult::new("Task completed successfully");

        let json = serde_json::to_string(&result).unwrap();

        // Empty handoff should be omitted
        assert!(!json.contains("\"handoff\""));
        assert!(json.contains("\"summary\":\"Task completed successfully\""));

        let deserialized: TaskResult = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.summary, "Task completed successfully");
        assert!(deserialized.handoff.is_empty());
    }

    #[test]
    fn test_handoff_is_empty() {
        let empty = Handoff::default();
        assert!(empty.is_empty());

        let with_decisions = Handoff {
            decisions: vec!["Chose X because Y".to_string()],
            ..Default::default()
        };
        assert!(!with_decisions.is_empty());

        let with_facts = Handoff {
            facts: vec!["Found error in file X".to_string()],
            ..Default::default()
        };
        assert!(!with_facts.is_empty());

        let with_next_steps = Handoff {
            next_steps: vec!["Do something".to_string()],
            ..Default::default()
        };
        assert!(!with_next_steps.is_empty());

        let with_blockers = Handoff {
            blockers: vec!["Blocked on X".to_string()],
            ..Default::default()
        };
        assert!(!with_blockers.is_empty());

        let with_files = Handoff {
            files_read: vec!["src/main.rs".to_string()],
            ..Default::default()
        };
        assert!(!with_files.is_empty());

        let with_resources = Handoff {
            resources: vec!["https://example.com - docs".to_string()],
            ..Default::default()
        };
        assert!(!with_resources.is_empty());
    }

    #[test]
    fn test_task_with_structured_result() {
        let id = TaskId::new_root("a1b2c3d4");
        let mut task = Task::new_root(id, "Research task".to_string());

        task.result = Some(TaskResult {
            summary: "Found the answer".to_string(),
            handoff: Handoff {
                facts: vec!["Key discovery".to_string()],
                ..Default::default()
            },
        });

        let json = serde_json::to_string(&task).unwrap();
        assert!(json.contains("\"result\":{"));
        assert!(json.contains("\"summary\":\"Found the answer\""));

        let deserialized: Task = serde_json::from_str(&json).unwrap();
        assert!(deserialized.result.is_some());
        assert_eq!(deserialized.result.unwrap().summary, "Found the answer");
    }
}
