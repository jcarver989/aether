use crate::render_context::{Component, RenderContext};
use crate::screen::Line;
use crossterm::style::Stylize;
use llm::{ToolCallError, ToolCallRequest, ToolCallResult};
use std::collections::HashMap;

const MAX_TOOL_ARG_LENGTH: usize = 200;

/// Renders a single tool call status line.
pub struct ToolCallStatusView {
    pub name: String,
    pub arguments: String,
    pub status: ToolCallStatus,
}

pub enum ToolCallStatus {
    Running,
    Success,
    Error(String),
}

impl Component for ToolCallStatusView {
    fn render(&self, context: &RenderContext) -> Vec<Line> {
        let (suffix, indicator_color) = match &self.status {
            ToolCallStatus::Running => ("", context.theme.info),
            ToolCallStatus::Success => (" ✓", context.theme.success),
            ToolCallStatus::Error(_) => (" X", context.theme.error),
        };

        let name_styled = format!("● {}{}", self.name, suffix).with(indicator_color);
        let args = Self::format_arguments(&self.arguments, context);

        let mut line_text = format!("{name_styled}{args}");

        if let ToolCallStatus::Error(msg) = &self.status {
            let error_styled = msg.to_string().with(context.theme.error);
            line_text.push_str(&format!(" {error_styled}"));
        }

        vec![Line::new(line_text)]
    }
}

impl ToolCallStatusView {
    fn format_arguments(arguments: &str, context: &RenderContext) -> String {
        let mut formatted = format!(" {arguments}");
        formatted.truncate(MAX_TOOL_ARG_LENGTH);
        format!("{}", formatted.with(context.theme.muted))
    }
}

/// Tracks active tool calls and produces status lines for the frame.
#[derive(Clone)]
pub struct ToolCallStatuses {
    /// Ordered list of tool call IDs (insertion order)
    tool_order: Vec<String>,
    /// Tool call info by ID
    tool_calls: HashMap<String, TrackedToolCall>,
}

#[derive(Clone)]
struct TrackedToolCall {
    name: String,
    arguments: String,
    status: TrackedStatus,
}

#[derive(Clone)]
enum TrackedStatus {
    Running,
    Success,
    Error(String),
}

impl ToolCallStatuses {
    pub fn new() -> Self {
        Self {
            tool_order: Vec::new(),
            tool_calls: HashMap::new(),
        }
    }

    pub fn on_tool_request(&mut self, request: &ToolCallRequest) {
        if let Some(existing) = self.tool_calls.get_mut(&request.id) {
            if !request.name.is_empty() {
                existing.name = request.name.clone();
            }

            if !request.arguments.is_empty() {
                if request.name.is_empty() {
                    existing.arguments.push_str(&request.arguments);
                } else {
                    existing.arguments = request.arguments.clone();
                }
            }
            return;
        }

        self.tool_order.push(request.id.clone());
        self.tool_calls.insert(
            request.id.clone(),
            TrackedToolCall {
                name: request.name.clone(),
                arguments: request.arguments.clone(),
                status: TrackedStatus::Running,
            },
        );
    }

    pub fn on_tool_result(&mut self, result: &ToolCallResult) {
        if let Some(tc) = self.tool_calls.get_mut(&result.id) {
            tc.arguments = result.arguments.clone();
            tc.status = TrackedStatus::Success;
        }
    }

    pub fn on_tool_error(&mut self, error: &ToolCallError) {
        if let Some(tc) = self.tool_calls.get_mut(&error.id) {
            if let Some(args) = &error.arguments {
                tc.arguments = args.clone();
            }
            tc.status = TrackedStatus::Error(error.error.clone());
        }
    }

    /// Clear all tracked tool calls (e.g., after pushing to scrollback).
    #[allow(dead_code)]
    pub fn clear(&mut self) {
        self.tool_order.clear();
        self.tool_calls.clear();
    }

    /// Render and remove only completed (Success/Error) tool calls,
    /// leaving Running ones in place for continued display.
    pub fn drain_completed(&mut self, context: &RenderContext) -> Vec<Line> {
        let mut lines = Vec::new();
        let mut completed_ids = Vec::new();

        for id in &self.tool_order {
            if let Some(tc) = self.tool_calls.get(id) {
                match &tc.status {
                    TrackedStatus::Running => continue,
                    TrackedStatus::Success => {
                        let view = ToolCallStatusView {
                            name: tc.name.clone(),
                            arguments: tc.arguments.clone(),
                            status: ToolCallStatus::Success,
                        };
                        lines.extend(view.render(context));
                        completed_ids.push(id.clone());
                    }
                    TrackedStatus::Error(msg) => {
                        let view = ToolCallStatusView {
                            name: tc.name.clone(),
                            arguments: tc.arguments.clone(),
                            status: ToolCallStatus::Error(msg.clone()),
                        };
                        lines.extend(view.render(context));
                        completed_ids.push(id.clone());
                    }
                }
            }
        }

        for id in &completed_ids {
            self.tool_calls.remove(id);
        }
        self.tool_order.retain(|id| !completed_ids.contains(id));

        lines
    }

    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.tool_calls.is_empty()
    }
}

impl Default for ToolCallStatuses {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for ToolCallStatuses {
    fn render(&self, context: &RenderContext) -> Vec<Line> {
        let mut lines = Vec::new();
        for id in &self.tool_order {
            if let Some(tc) = self.tool_calls.get(id) {
                let status = match &tc.status {
                    TrackedStatus::Running => ToolCallStatus::Running,
                    TrackedStatus::Success => ToolCallStatus::Success,
                    TrackedStatus::Error(msg) => ToolCallStatus::Error(msg.clone()),
                };
                let view = ToolCallStatusView {
                    name: tc.name.clone(),
                    arguments: tc.arguments.clone(),
                    status,
                };
                lines.extend(view.render(context));
            }
        }
        lines
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx() -> RenderContext {
        RenderContext::new((80, 24))
    }

    #[test]
    fn request_tracks_tool() {
        let mut statuses = ToolCallStatuses::new();
        let request = ToolCallRequest {
            id: "tool-1".to_string(),
            name: "Read".to_string(),
            arguments: "/path/to/file".to_string(),
        };
        statuses.on_tool_request(&request);
        let lines = statuses.render(&ctx());
        assert_eq!(lines.len(), 1);
        assert!(lines[0].as_str().contains("Read"));
    }

    #[test]
    fn duplicate_request_updates_existing_tool() {
        let mut statuses = ToolCallStatuses::new();
        statuses.on_tool_request(&ToolCallRequest {
            id: "tool-1".to_string(),
            name: "Read".to_string(),
            arguments: "".to_string(),
        });

        // Streaming arg chunks for same ID should be merged.
        statuses.on_tool_request(&ToolCallRequest {
            id: "tool-1".to_string(),
            name: "".to_string(),
            arguments: "{\"file\":".to_string(),
        });
        statuses.on_tool_request(&ToolCallRequest {
            id: "tool-1".to_string(),
            name: "".to_string(),
            arguments: "\"test.rs\"}".to_string(),
        });

        let lines = statuses.render(&ctx());
        assert_eq!(lines.len(), 1);
        assert!(lines[0].as_str().contains("Read"));
        assert!(lines[0].as_str().contains("{\"file\":\"test.rs\"}"));
    }

    #[test]
    fn result_updates_to_success() {
        let mut statuses = ToolCallStatuses::new();
        statuses.on_tool_request(&ToolCallRequest {
            id: "tool-1".to_string(),
            name: "Read".to_string(),
            arguments: "".to_string(),
        });
        statuses.on_tool_result(&ToolCallResult {
            id: "tool-1".to_string(),
            name: "Read".to_string(),
            arguments: "success".to_string(),
            result: "contents".to_string(),
        });
        let lines = statuses.render(&ctx());
        assert_eq!(lines.len(), 1);
        assert!(lines[0].as_str().contains("✓"));
    }

    #[test]
    fn unknown_result_is_ignored() {
        let mut statuses = ToolCallStatuses::new();
        statuses.on_tool_result(&ToolCallResult {
            id: "unknown".to_string(),
            name: "Read".to_string(),
            arguments: "".to_string(),
            result: "".to_string(),
        });
        let lines = statuses.render(&ctx());
        assert!(lines.is_empty());
    }

    #[test]
    fn error_updates_to_error_state() {
        let mut statuses = ToolCallStatuses::new();
        statuses.on_tool_request(&ToolCallRequest {
            id: "tool-1".to_string(),
            name: "Read".to_string(),
            arguments: "".to_string(),
        });
        statuses.on_tool_error(&ToolCallError {
            id: "tool-1".to_string(),
            name: "Read".to_string(),
            arguments: None,
            error: "not found".to_string(),
        });
        let lines = statuses.render(&ctx());
        assert_eq!(lines.len(), 1);
        assert!(lines[0].as_str().contains("X"));
        assert!(lines[0].as_str().contains("not found"));
    }

    #[test]
    fn multiple_tools_render_in_order() {
        let mut statuses = ToolCallStatuses::new();
        statuses.on_tool_request(&ToolCallRequest {
            id: "tool-1".to_string(),
            name: "Read".to_string(),
            arguments: "".to_string(),
        });
        statuses.on_tool_request(&ToolCallRequest {
            id: "tool-2".to_string(),
            name: "Write".to_string(),
            arguments: "".to_string(),
        });
        let lines = statuses.render(&ctx());
        assert_eq!(lines.len(), 2);
        assert!(lines[0].as_str().contains("Read"));
        assert!(lines[1].as_str().contains("Write"));
    }

    #[test]
    fn multiple_tools_complete_independently() {
        let mut statuses = ToolCallStatuses::new();
        statuses.on_tool_request(&ToolCallRequest {
            id: "tool-1".to_string(),
            name: "Read".to_string(),
            arguments: "".to_string(),
        });
        statuses.on_tool_request(&ToolCallRequest {
            id: "tool-2".to_string(),
            name: "Write".to_string(),
            arguments: "".to_string(),
        });
        statuses.on_tool_result(&ToolCallResult {
            id: "tool-1".to_string(),
            name: "Read".to_string(),
            arguments: "".to_string(),
            result: "".to_string(),
        });
        let lines = statuses.render(&ctx());
        assert_eq!(lines.len(), 2);
        assert!(lines[0].as_str().contains("✓")); // Read completed
        assert!(!lines[1].as_str().contains("✓")); // Write still running
    }

    #[test]
    fn clear_removes_all() {
        let mut statuses = ToolCallStatuses::new();
        statuses.on_tool_request(&ToolCallRequest {
            id: "tool-1".to_string(),
            name: "Read".to_string(),
            arguments: "".to_string(),
        });
        statuses.clear();
        assert!(statuses.is_empty());
        assert!(statuses.render(&ctx()).is_empty());
    }

    #[test]
    fn drain_completed_only_removes_success_and_error() {
        let mut statuses = ToolCallStatuses::new();

        // Add a Running tool call
        statuses.on_tool_request(&ToolCallRequest {
            id: "tool-1".to_string(),
            name: "Read".to_string(),
            arguments: "file.rs".to_string(),
        });

        // Add a Success tool call
        statuses.on_tool_request(&ToolCallRequest {
            id: "tool-2".to_string(),
            name: "Write".to_string(),
            arguments: "out.rs".to_string(),
        });
        statuses.on_tool_result(&ToolCallResult {
            id: "tool-2".to_string(),
            name: "Write".to_string(),
            arguments: "out.rs".to_string(),
            result: "ok".to_string(),
        });

        // Add an Error tool call
        statuses.on_tool_request(&ToolCallRequest {
            id: "tool-3".to_string(),
            name: "Grep".to_string(),
            arguments: "pattern".to_string(),
        });
        statuses.on_tool_error(&ToolCallError {
            id: "tool-3".to_string(),
            name: "Grep".to_string(),
            arguments: None,
            error: "not found".to_string(),
        });

        // Drain should return only the completed (Success + Error) tool calls
        let drained = statuses.drain_completed(&ctx());
        assert_eq!(drained.len(), 2); // Write (success) + Grep (error)
        assert!(drained[0].as_str().contains("Write"));
        assert!(drained[0].as_str().contains("✓"));
        assert!(drained[1].as_str().contains("Grep"));
        assert!(drained[1].as_str().contains("X"));

        // Running tool call should still be tracked
        let remaining = statuses.render(&ctx());
        assert_eq!(remaining.len(), 1);
        assert!(remaining[0].as_str().contains("Read"));
    }

    #[test]
    fn drain_completed_with_no_completed_returns_empty() {
        let mut statuses = ToolCallStatuses::new();
        statuses.on_tool_request(&ToolCallRequest {
            id: "tool-1".to_string(),
            name: "Read".to_string(),
            arguments: "".to_string(),
        });
        statuses.on_tool_request(&ToolCallRequest {
            id: "tool-2".to_string(),
            name: "Write".to_string(),
            arguments: "".to_string(),
        });

        let drained = statuses.drain_completed(&ctx());
        assert!(drained.is_empty());

        // Both should still be tracked
        let remaining = statuses.render(&ctx());
        assert_eq!(remaining.len(), 2);
    }

    #[test]
    fn drain_completed_allows_late_arriving_results() {
        let mut statuses = ToolCallStatuses::new();

        // Two tool calls, one completes
        statuses.on_tool_request(&ToolCallRequest {
            id: "tool-1".to_string(),
            name: "Read".to_string(),
            arguments: "".to_string(),
        });
        statuses.on_tool_request(&ToolCallRequest {
            id: "tool-2".to_string(),
            name: "Write".to_string(),
            arguments: "".to_string(),
        });
        statuses.on_tool_result(&ToolCallResult {
            id: "tool-1".to_string(),
            name: "Read".to_string(),
            arguments: "".to_string(),
            result: "ok".to_string(),
        });

        // Drain the completed one
        statuses.drain_completed(&ctx());

        // Late result for remaining Running call should still work
        statuses.on_tool_result(&ToolCallResult {
            id: "tool-2".to_string(),
            name: "Write".to_string(),
            arguments: "done".to_string(),
            result: "ok".to_string(),
        });

        let remaining = statuses.render(&ctx());
        assert_eq!(remaining.len(), 1);
        assert!(remaining[0].as_str().contains("Write"));
        assert!(remaining[0].as_str().contains("✓"));
    }

    #[test]
    fn view_renders_running() {
        let view = ToolCallStatusView {
            name: "TestTool".to_string(),
            arguments: "test args".to_string(),
            status: ToolCallStatus::Running,
        };
        let lines = view.render(&ctx());
        assert_eq!(lines.len(), 1);
        assert!(lines[0].as_str().contains("TestTool"));
        assert!(lines[0].as_str().contains("test args"));
    }

    #[test]
    fn view_renders_success() {
        let view = ToolCallStatusView {
            name: "TestTool".to_string(),
            arguments: "test args".to_string(),
            status: ToolCallStatus::Success,
        };
        let lines = view.render(&ctx());
        assert_eq!(lines.len(), 1);
        assert!(lines[0].as_str().contains("✓"));
    }

    #[test]
    fn view_renders_error() {
        let view = ToolCallStatusView {
            name: "TestTool".to_string(),
            arguments: "test args".to_string(),
            status: ToolCallStatus::Error("boom".to_string()),
        };
        let lines = view.render(&ctx());
        assert_eq!(lines.len(), 1);
        assert!(lines[0].as_str().contains("X"));
        assert!(lines[0].as_str().contains("boom"));
    }
}
