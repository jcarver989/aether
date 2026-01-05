use crate::components::tool_display::types::{TodoDisplayMeta, TodoItemMeta};
use dioxus::prelude::*;

/// Display a todo/task list in a human-friendly way.
#[component]
pub fn TodoDisplay(todo_meta: TodoDisplayMeta) -> Element {
    let progress = todo_meta.progress_label();

    rsx! {
        div {
            class: "flex flex-col gap-1 py-1",

            div {
                class: "flex items-center gap-2 mb-1",
                span { class: "text-purple-400 text-xs", "✓" }
                span { class: "text-sm text-gray-300", "Tasks" }
                if let Some(progress) = &progress {
                    span {
                        class: "text-xs text-gray-500",
                        "{progress}"
                    }
                }
            }

            div {
                class: "flex flex-col gap-1 ml-5",
                for item in &todo_meta.items {
                    div {
                        class: "flex items-start gap-2",
                        span {
                            class: if item.completed {
                                "text-xs text-green-400 line-through opacity-60"
                            } else {
                                "text-xs text-gray-400"
                            },
                            if item.completed { "✓" } else { "○" }
                        }
                        span {
                            class: "text-sm text-gray-300",
                            "{todo_item_label(item)}"
                        }
                    }
                }
            }
        }
    }
}

fn todo_item_label(item: &TodoItemMeta) -> &str {
    if item.completed {
        item.content.as_str()
    } else {
        item.active_form.as_deref().unwrap_or(item.content.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::tool_display::types::TodoItemMeta;

    #[test]
    fn test_todo_display_all_completed() {
        let meta = TodoDisplayMeta {
            items: vec![
                TodoItemMeta {
                    content: "Task 1".to_string(),
                    completed: true,
                    active_form: Some("Completing task 1".to_string()),
                },
                TodoItemMeta {
                    content: "Task 2".to_string(),
                    completed: true,
                    active_form: Some("Completing task 2".to_string()),
                },
            ],
        };

        assert_eq!(meta.items.len(), 2);
        assert_eq!(meta.items.iter().filter(|i| i.completed).count(), 2);
    }

    #[test]
    fn test_todo_display_mixed() {
        let meta = TodoDisplayMeta {
            items: vec![
                TodoItemMeta {
                    content: "Task 1".to_string(),
                    completed: false,
                    active_form: Some("Working on task 1".to_string()),
                },
                TodoItemMeta {
                    content: "Task 2".to_string(),
                    completed: true,
                    active_form: None,
                },
            ],
        };

        assert_eq!(meta.items.len(), 2);
        assert_eq!(meta.items.iter().filter(|i| i.completed).count(), 1);
    }

    #[test]
    fn test_todo_item_active_form() {
        let item = TodoItemMeta {
            content: "Create a file".to_string(),
            completed: false,
            active_form: Some("Creating file".to_string()),
        };

        assert_eq!(item.content, "Create a file");
        assert_eq!(item.active_form, Some("Creating file".to_string()));
        assert!(!item.completed);
    }
}
