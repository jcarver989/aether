//! Task management for deep research agents.
//!
//! This module provides persistent task storage with support for:
//! - Hierarchical task trees (root tasks with subtasks)
//! - Dependency tracking between tasks
//! - Multi-agent coordination via assignees
//! - Git-friendly JSONL file storage
//!
//! # Storage Structure
//!
//! Tasks are stored in `.aether-tasks/` as JSONL files:
//! ```text
//! .aether-tasks/
//!   active/
//!     at-a1b2.jsonl    # Root task + all subtasks as lines
//!     at-c3d4.jsonl
//!   completed/
//!     at-old1.jsonl
//! ```
//!
//! # Example
//!
//! ```rust,ignore
//! use mcp_lexicon::coding::tools::tasks::{TaskStore, TaskUpdate, TaskStatus};
//!
//! let mut store = TaskStore::new(".aether-tasks".into());
//! store.init()?;
//!
//! // Create a research task tree
//! let root = store.create_tree("Research topic X", Some("Investigate..."))?;
//!
//! // Add subtasks
//! let sub1 = store.add_subtask(&root.id, "Gather sources")?;
//! let sub2 = store.add_subtask(&root.id, "Analyze findings")?;
//!
//! // Update with dependency
//! store.update(&sub2.id, TaskUpdate {
//!     deps: Some(vec![sub1.id.clone()]),
//!     ..Default::default()
//! })?;
//!
//! // Complete tasks
//! store.complete(&sub1.id, Some("Found 5 relevant papers"))?;
//! ```

mod index;
mod store;
mod types;

mod tool_complete;
mod tool_create;
mod tool_list;
mod tool_update;

pub use index::TaskIndex;
pub use store::{TaskStore, TaskStoreError};
pub use types::{Task, TaskId, TaskStatus, TaskUpdate};

pub use tool_complete::{TaskCompleteInput, TaskCompleteOutput, execute_task_complete};
pub use tool_create::{TaskCreateInput, TaskCreateOutput, execute_task_create};
pub use tool_list::{TaskListInput, TaskListOutput, execute_task_list};
pub use tool_update::{TaskUpdateInput, TaskUpdateOutput, execute_task_update};
