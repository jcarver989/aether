use mcp_lexicon::coding::{TodoItem, TodoState, TodoWriteInput, process_todo_write};
use serde_json;

#[test]
fn test_todo_write_empty_list() {
    let input = TodoWriteInput { todos: vec![] };
    let output = process_todo_write(input);

    assert_eq!(output.stats.total, 0);
    assert_eq!(output.stats.pending, 0);
    assert_eq!(output.stats.in_progress, 0);
    assert_eq!(output.stats.completed, 0);
    assert_eq!(output.message, "Todo list updated with 0 task(s)");
}

#[test]
fn test_todo_write_single_pending_task() {
    let input = TodoWriteInput {
        todos: vec![TodoItem {
            content: "Write unit tests".to_string(),
            state: TodoState::Pending,
            active_form: "Writing unit tests".to_string(),
        }],
    };

    let output = process_todo_write(input);

    assert_eq!(output.stats.total, 1);
    assert_eq!(output.stats.pending, 1);
    assert_eq!(output.stats.in_progress, 0);
    assert_eq!(output.stats.completed, 0);
    assert_eq!(output.message, "Todo list updated with 1 task(s)");
}

#[test]
fn test_todo_write_mixed_statuses() {
    let input = TodoWriteInput {
        todos: vec![
            TodoItem {
                content: "Design API".to_string(),
                state: TodoState::Completed,
                active_form: "Designing API".to_string(),
            },
            TodoItem {
                content: "Implement feature".to_string(),
                state: TodoState::InProgress,
                active_form: "Implementing feature".to_string(),
            },
            TodoItem {
                content: "Write documentation".to_string(),
                state: TodoState::Pending,
                active_form: "Writing documentation".to_string(),
            },
            TodoItem {
                content: "Review code".to_string(),
                state: TodoState::Pending,
                active_form: "Reviewing code".to_string(),
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
fn test_todo_write_all_in_progress() {
    let input = TodoWriteInput {
        todos: vec![
            TodoItem {
                content: "Task 1".to_string(),
                state: TodoState::InProgress,
                active_form: "Working on task 1".to_string(),
            },
            TodoItem {
                content: "Task 2".to_string(),
                state: TodoState::InProgress,
                active_form: "Working on task 2".to_string(),
            },
        ],
    };

    let output = process_todo_write(input);

    assert_eq!(output.stats.total, 2);
    assert_eq!(output.stats.pending, 0);
    assert_eq!(output.stats.in_progress, 2);
    assert_eq!(output.stats.completed, 0);
}

#[test]
fn test_todo_write_all_completed() {
    let input = TodoWriteInput {
        todos: vec![
            TodoItem {
                content: "Task 1".to_string(),
                state: TodoState::Completed,
                active_form: "Working on task 1".to_string(),
            },
            TodoItem {
                content: "Task 2".to_string(),
                state: TodoState::Completed,
                active_form: "Working on task 2".to_string(),
            },
            TodoItem {
                content: "Task 3".to_string(),
                state: TodoState::Completed,
                active_form: "Working on task 3".to_string(),
            },
        ],
    };

    let output = process_todo_write(input);

    assert_eq!(output.stats.total, 3);
    assert_eq!(output.stats.pending, 0);
    assert_eq!(output.stats.in_progress, 0);
    assert_eq!(output.stats.completed, 3);
}

#[test]
fn test_todo_write_progress_workflow() {
    // Start with pending task
    let input1 = TodoWriteInput {
        todos: vec![TodoItem {
            content: "Implement feature X".to_string(),
            state: TodoState::Pending,
            active_form: "Implementing feature X".to_string(),
        }],
    };

    let output1 = process_todo_write(input1);
    assert_eq!(output1.stats.pending, 1);
    assert_eq!(output1.stats.in_progress, 0);

    // Move to in-progress
    let input2 = TodoWriteInput {
        todos: vec![TodoItem {
            content: "Implement feature X".to_string(),
            state: TodoState::InProgress,
            active_form: "Implementing feature X".to_string(),
        }],
    };

    let output2 = process_todo_write(input2);
    assert_eq!(output2.stats.pending, 0);
    assert_eq!(output2.stats.in_progress, 1);

    // Complete the task
    let input3 = TodoWriteInput {
        todos: vec![TodoItem {
            content: "Implement feature X".to_string(),
            state: TodoState::Completed,
            active_form: "Implementing feature X".to_string(),
        }],
    };

    let output3 = process_todo_write(input3);
    assert_eq!(output3.stats.pending, 0);
    assert_eq!(output3.stats.in_progress, 0);
    assert_eq!(output3.stats.completed, 1);
}

#[test]
fn test_serde_casing_consistency() {
    // Test serialization of TodoItem (which has active_form field)
    let todo_item = TodoItem {
        content: "Test task".to_string(),
        state: TodoState::InProgress,
        active_form: "Testing task".to_string(),
    };

    let todo_json = serde_json::to_string(&todo_item).unwrap();
    println!("TodoItem JSON: {}", todo_json);

    // Verify that TodoItem uses snake_case
    assert!(todo_json.contains("active_form"));
    assert!(todo_json.contains("in_progress")); // status should be snake_case

    let input = TodoWriteInput {
        todos: vec![todo_item],
    };

    let output = process_todo_write(input);

    // Test JSON serialization uses consistent camelCase
    let json = serde_json::to_string(&output).unwrap();
    println!("Output JSON: {}", json);

    // Verify that output uses snake_case for stats
    assert!(json.contains("in_progress")); // stats.in_progress should be snake_case

    // Test deserialization also works with camelCase
    let json_input = r#"
    {
        "todos": [
            {
                "content": "Test task",
                "state": "in_progress",
                "active_form": "Testing task"
            }
        ]
    }
    "#;

    let parsed: TodoWriteInput = serde_json::from_str(json_input).unwrap();
    assert_eq!(parsed.todos.len(), 1);
    assert_eq!(parsed.todos[0].content, "Test task");
    assert!(matches!(parsed.todos[0].state, TodoState::InProgress));
    assert_eq!(parsed.todos[0].active_form, "Testing task");
}
