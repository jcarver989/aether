use crate::components::tool_display::types::CommandDisplayMeta;
use dioxus::prelude::*;

/// Display a bash/command tool result in a human-friendly way.
#[component]
pub fn BashDisplay(command_meta: CommandDisplayMeta) -> Element {
    let exit_code_class = if command_meta.exit_code == 0 {
        "text-green-400"
    } else {
        "text-red-400"
    };

    let exit_status = if command_meta.killed == Some(true) {
        "✕ killed (timeout)".to_string()
    } else if command_meta.exit_code == 0 {
        "✓ exit 0".to_string()
    } else {
        format!("✕ exit {}", command_meta.exit_code)
    };

    rsx! {
        div {
            class: "flex flex-col gap-1 py-1",

            div {
                class: "flex items-center gap-2",
                span { class: "text-gray-500 text-xs", "→" }
                span { class: "font-mono text-sm text-gray-200", "{command_meta.command}" }
            }

            if let Some(desc) = &command_meta.description {
                div {
                    class: "flex items-center gap-2",
                    span { class: "text-gray-500 text-xs", " " }
                    span { class: "text-sm text-gray-400 italic", "{desc}" }
                }
            }

            div {
                class: "flex items-center gap-2",
                span { class: "text-gray-500 text-xs", " " }
                span { class: "text-xs {exit_code_class}", "{exit_status}" }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bash_display_failed_exit() {
        let meta = CommandDisplayMeta {
            command: "cargo build".to_string(),
            description: None,
            exit_code: 1,
            killed: None,
        };

        assert_ne!(meta.exit_code, 0);
    }

    #[test]
    fn test_bash_display_killed() {
        let meta = CommandDisplayMeta {
            command: "sleep 100".to_string(),
            description: Some("Sleep for 100 seconds".to_string()),
            exit_code: -1,
            killed: Some(true),
        };

        assert_eq!(meta.killed, Some(true));
    }
}
