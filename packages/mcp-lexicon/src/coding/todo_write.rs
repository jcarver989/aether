use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum TodoStatus {
    Pending,
    InProgress,
    Completed,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TodoItem {
    /// The task description
    pub content: String,
    /// The task status
    pub status: TodoStatus,
    /// Active form of the task description
    pub active_form: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct TodoWriteInput {
    /// The updated todo list
    pub todos: Vec<TodoItem>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct TodoStats {
    pub total: usize,
    pub pending: usize,
    pub in_progress: usize,
    pub completed: usize,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct TodoWriteOutput {
    /// Success message
    pub message: String,
    /// Current todo statistics
    pub stats: TodoStats,
}

/// Process the todo write operation and return statistics
pub fn process_todo_write(input: TodoWriteInput) -> TodoWriteOutput {
    let todos = &input.todos;

    let total = todos.len();
    let pending = todos.iter().filter(|t| matches!(t.status, TodoStatus::Pending)).count();
    let in_progress = todos.iter().filter(|t| matches!(t.status, TodoStatus::InProgress)).count();
    let completed = todos.iter().filter(|t| matches!(t.status, TodoStatus::Completed)).count();

    TodoWriteOutput {
        message: format!("Todo list updated with {} task(s)", total),
        stats: TodoStats {
            total,
            pending,
            in_progress,
            completed,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_process_todo_write_empty() {
        let input = TodoWriteInput { todos: vec![] };
        let output = process_todo_write(input);

        assert_eq!(output.stats.total, 0);
        assert_eq!(output.stats.pending, 0);
        assert_eq!(output.stats.in_progress, 0);
        assert_eq!(output.stats.completed, 0);
    }

    #[test]
    fn test_process_todo_write_mixed_statuses() {
        let input = TodoWriteInput {
            todos: vec![
                TodoItem {
                    content: "Task 1".to_string(),
                    status: TodoStatus::Pending,
                    active_form: "Working on task 1".to_string(),
                },
                TodoItem {
                    content: "Task 2".to_string(),
                    status: TodoStatus::InProgress,
                    active_form: "Working on task 2".to_string(),
                },
                TodoItem {
                    content: "Task 3".to_string(),
                    status: TodoStatus::Completed,
                    active_form: "Working on task 3".to_string(),
                },
                TodoItem {
                    content: "Task 4".to_string(),
                    status: TodoStatus::Pending,
                    active_form: "Working on task 4".to_string(),
                },
            ],
        };

        let output = process_todo_write(input);

        assert_eq!(output.stats.total, 4);
        assert_eq!(output.stats.pending, 2);
        assert_eq!(output.stats.in_progress, 1);
        assert_eq!(output.stats.completed, 1);
        assert_eq!(output.message, "Todo list updated with 4 task(s)");
    }

    #[test]
    fn test_process_todo_write_all_completed() {
        let input = TodoWriteInput {
            todos: vec![
                TodoItem {
                    content: "Task 1".to_string(),
                    status: TodoStatus::Completed,
                    active_form: "Working on task 1".to_string(),
                },
                TodoItem {
                    content: "Task 2".to_string(),
                    status: TodoStatus::Completed,
                    active_form: "Working on task 2".to_string(),
                },
            ],
        };

        let output = process_todo_write(input);

        assert_eq!(output.stats.total, 2);
        assert_eq!(output.stats.pending, 0);
        assert_eq!(output.stats.in_progress, 0);
        assert_eq!(output.stats.completed, 2);
    }
}
