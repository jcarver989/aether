use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::hash::Hash;

// ============================================================================
// Structured Task Result Types
// ============================================================================

/// Structured task completion result aligned with compression-resilient probe types.
///
/// This schema captures information that survives context compression, focusing on
/// what git cannot track: decisions, findings, continuation context, and resources.
/// File modifications are tracked by git (`git diff --name-only`).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct TaskResult {
    /// Executive summary (1-3 sentences)
    pub summary: String,

    /// Key decisions made during task execution
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub decisions: Vec<Decision>,

    /// Critical facts discovered (error messages, config values, patterns)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub findings: Vec<Finding>,

    /// Continuation context for downstream tasks
    #[serde(default, skip_serializing_if = "Continuation::is_empty")]
    pub continuation: Continuation,

    /// Files read but not modified (git doesn't track reads)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub files_read: Vec<String>,

    /// External resources accessed (git doesn't track these)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub resources: Vec<ResourceRef>,
}

/// A decision made during task execution, preserving the reasoning chain.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct Decision {
    /// What was decided
    pub what: String,

    /// Why this choice was made
    pub why: String,

    /// Alternatives considered but rejected
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rejected: Vec<String>,
}

/// A factual finding discovered during task execution.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct Finding {
    /// Category: "error", "config", "pattern", "constraint", etc.
    pub kind: String,

    /// The actual finding
    pub content: String,

    /// Where this was found (file path, command output, etc.)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

/// Continuation context for downstream tasks.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct Continuation {
    /// Suggested next steps identified during execution
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub next_steps: Vec<String>,

    /// Blockers or dependencies that couldn't be resolved
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub blockers: Vec<String>,

    /// Open questions needing human input or further research
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub open_questions: Vec<String>,
}

impl Continuation {
    pub fn is_empty(&self) -> bool {
        self.next_steps.is_empty() && self.blockers.is_empty() && self.open_questions.is_empty()
    }
}

/// Reference to an external resource accessed during task execution.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct ResourceRef {
    /// URI (URL, API endpoint, database connection, etc.)
    pub uri: String,

    /// What was retrieved or learned from this resource
    pub summary: String,
}

// ============================================================================
// Task ID and Core Types
// ============================================================================

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

    /// Structured findings/output when completed (preferred)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<TaskResult>,

    /// Simple text result for backwards compatibility or trivial tasks
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result_text: Option<String>,

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
            result_text: None,
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
            result_text: None,
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
}

impl TaskUpdate {
    /// Apply updates to a task
    pub fn apply_to(&self, task: &mut Task) {
        if let Some(title) = &self.title {
            task.title = title.clone();
        }
        if let Some(description) = &self.description {
            task.description = Some(description.clone());
        }
        if let Some(status) = self.status {
            task.status = status;
        }
        if let Some(assignee) = &self.assignee {
            task.assignee = Some(assignee.clone());
        }
        if let Some(deps) = &self.deps {
            task.deps = deps.clone();
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

        // Not ready - no deps completed
        assert!(!task.is_ready(&[]));

        // Not ready - only one dep completed
        assert!(!task.is_ready(&[&id1]));

        // Ready - all deps completed
        assert!(task.is_ready(&[&id1, &id2]));

        // Not ready if not pending
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
            decisions: vec![Decision {
                what: "Defer JWT migration".to_string(),
                why: "Breaking change requires SDK updates".to_string(),
                rejected: vec!["Immediate migration".to_string()],
            }],
            findings: vec![Finding {
                kind: "pattern".to_string(),
                content: "All endpoints use validate_session()".to_string(),
                source: Some("src/api/*.rs".to_string()),
            }],
            continuation: Continuation {
                next_steps: vec!["Create migration guide".to_string()],
                blockers: vec!["Need product decision".to_string()],
                open_questions: vec!["Support both auth methods?".to_string()],
            },
            files_read: vec!["src/api/auth.rs".to_string()],
            resources: vec![ResourceRef {
                uri: "https://docs.rs/jsonwebtoken".to_string(),
                summary: "JWT library docs".to_string(),
            }],
        };

        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"summary\":\"Identified 5 API endpoints"));
        assert!(json.contains("\"decisions\":["));
        assert!(json.contains("\"findings\":["));
        assert!(json.contains("\"continuation\":{"));
        assert!(json.contains("\"files_read\":["));
        assert!(json.contains("\"resources\":["));

        let deserialized: TaskResult = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.summary, result.summary);
        assert_eq!(deserialized.decisions.len(), 1);
        assert_eq!(deserialized.findings.len(), 1);
        assert_eq!(deserialized.continuation.next_steps.len(), 1);
    }

    #[test]
    fn test_task_result_minimal_serialization() {
        let result = TaskResult {
            summary: "Task completed successfully".to_string(),
            decisions: vec![],
            findings: vec![],
            continuation: Continuation::default(),
            files_read: vec![],
            resources: vec![],
        };

        let json = serde_json::to_string(&result).unwrap();

        // Empty fields should be skipped
        assert!(!json.contains("\"decisions\""));
        assert!(!json.contains("\"findings\""));
        assert!(!json.contains("\"continuation\""));
        assert!(!json.contains("\"files_read\""));
        assert!(!json.contains("\"resources\""));

        // Only summary should be present
        assert!(json.contains("\"summary\":\"Task completed successfully\""));

        let deserialized: TaskResult = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.summary, "Task completed successfully");
        assert!(deserialized.decisions.is_empty());
    }

    #[test]
    fn test_continuation_is_empty() {
        let empty = Continuation::default();
        assert!(empty.is_empty());

        let with_next_steps = Continuation {
            next_steps: vec!["Do something".to_string()],
            ..Default::default()
        };
        assert!(!with_next_steps.is_empty());

        let with_blockers = Continuation {
            blockers: vec!["Blocked on X".to_string()],
            ..Default::default()
        };
        assert!(!with_blockers.is_empty());

        let with_questions = Continuation {
            open_questions: vec!["Should we?".to_string()],
            ..Default::default()
        };
        assert!(!with_questions.is_empty());
    }

    #[test]
    fn test_finding_without_source() {
        let finding = Finding {
            kind: "error".to_string(),
            content: "Connection timeout".to_string(),
            source: None,
        };

        let json = serde_json::to_string(&finding).unwrap();
        assert!(!json.contains("\"source\""));

        let deserialized: Finding = serde_json::from_str(&json).unwrap();
        assert!(deserialized.source.is_none());
    }

    #[test]
    fn test_decision_without_rejected() {
        let decision = Decision {
            what: "Use async approach".to_string(),
            why: "Better performance".to_string(),
            rejected: vec![],
        };

        let json = serde_json::to_string(&decision).unwrap();
        assert!(!json.contains("\"rejected\""));

        let deserialized: Decision = serde_json::from_str(&json).unwrap();
        assert!(deserialized.rejected.is_empty());
    }

    #[test]
    fn test_task_with_structured_result() {
        let id = TaskId::new_root("a1b2c3d4");
        let mut task = Task::new_root(id, "Research task".to_string());

        task.result = Some(TaskResult {
            summary: "Found the answer".to_string(),
            decisions: vec![],
            findings: vec![Finding {
                kind: "insight".to_string(),
                content: "Key discovery".to_string(),
                source: None,
            }],
            continuation: Continuation::default(),
            files_read: vec![],
            resources: vec![],
        });

        let json = serde_json::to_string(&task).unwrap();
        assert!(json.contains("\"result\":{"));
        assert!(json.contains("\"summary\":\"Found the answer\""));

        let deserialized: Task = serde_json::from_str(&json).unwrap();
        assert!(deserialized.result.is_some());
        assert_eq!(deserialized.result.unwrap().summary, "Found the answer");
    }

    #[test]
    fn test_task_with_simple_result_text() {
        let id = TaskId::new_root("a1b2c3d4");
        let mut task = Task::new_root(id, "Simple task".to_string());

        task.result_text = Some("Fixed typo in README".to_string());

        let json = serde_json::to_string(&task).unwrap();
        assert!(json.contains("\"result_text\":\"Fixed typo in README\""));
        assert!(!json.contains("\"result\":{"));

        let deserialized: Task = serde_json::from_str(&json).unwrap();
        assert!(deserialized.result.is_none());
        assert_eq!(
            deserialized.result_text,
            Some("Fixed typo in README".to_string())
        );
    }
}
