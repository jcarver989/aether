//! Tasks MCP server for deep research agent workflows.
//!
//! This module provides a standalone MCP server for task management with support for:
//! - Hierarchical task trees (root tasks with subtasks)
//! - Dependency tracking between tasks
//! - Multi-agent coordination via assignees
//! - Git-friendly JSONL file storage
//!
//! # Usage
//!
//! ```rust,ignore
//! use mcp_lexicon::{TasksMcp, ServiceExt};
//!
//! let server = TasksMcp::new(".".into()).into_dyn();
//! ```

mod server;
mod task_index;
mod task_store;
mod types;

pub mod tools;

pub use server::{TasksMcp, TasksMcpArgs};
pub use task_index::TaskIndex;
pub use task_store::{TaskStore, TaskStoreError};
pub use tools::common::{HandoffDetail, TaskDetail, TaskResultDetail, TaskSummary};
pub use tools::create::{TaskCreateInput, TaskCreateOutput, execute_task_create};
pub use tools::get::{TaskGetInput, TaskGetOutput, execute_task_get};
pub use tools::list::{TaskListInput, TaskListOutput, execute_task_list};
pub use tools::update::{TaskUpdateInput, TaskUpdateOutput, execute_task_update};
pub use types::{Handoff, Task, TaskId, TaskResult, TaskStatus, TaskUpdate};
