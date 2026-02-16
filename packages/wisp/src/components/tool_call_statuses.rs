use agent_client_protocol as acp;

use crate::tui::{Component, Line, RenderContext};
use crossterm::style::Stylize;
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

    /// Handle a new tool call from ACP SessionUpdate::ToolCall.
    pub fn on_tool_call(&mut self, tool_call: &acp::ToolCall) {
        let id = tool_call.tool_call_id.0.to_string();
        let arguments = tool_call
            .raw_input
            .as_ref()
            .map(|v| v.to_string())
            .unwrap_or_default();

        if let Some(existing) = self.tool_calls.get_mut(&id) {
            if !tool_call.title.is_empty() {
                existing.name = tool_call.title.clone();
            }
            existing.arguments = arguments;
            return;
        }

        self.tool_order.push(id.clone());
        self.tool_calls.insert(
            id,
            TrackedToolCall {
                name: tool_call.title.clone(),
                arguments,
                status: TrackedStatus::Running,
            },
        );
    }

    /// Handle a tool call update from ACP SessionUpdate::ToolCallUpdate.
    pub fn on_tool_call_update(&mut self, update: &acp::ToolCallUpdate) {
        let id = update.tool_call_id.0.to_string();

        if let Some(tc) = self.tool_calls.get_mut(&id) {
            if let Some(title) = &update.fields.title {
                tc.name = title.clone();
            }
            if let Some(raw_input) = &update.fields.raw_input {
                tc.arguments = raw_input.to_string();
            }
            if let Some(status) = &update.fields.status {
                match status {
                    acp::ToolCallStatus::Completed => tc.status = TrackedStatus::Success,
                    acp::ToolCallStatus::Failed => {
                        tc.status = TrackedStatus::Error("failed".to_string());
                    }
                    acp::ToolCallStatus::InProgress | acp::ToolCallStatus::Pending => {
                        tc.status = TrackedStatus::Running;
                    }
                    _ => {}
                }
            }
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

    fn make_tool_call(id: &str, title: &str, raw_input: Option<&str>) -> acp::ToolCall {
        let mut tc = acp::ToolCall::new(id.to_string(), title);
        if let Some(input) = raw_input {
            tc = tc.raw_input(serde_json::from_str::<serde_json::Value>(input).unwrap());
        }
        tc
    }

    fn make_tool_call_update(id: &str, status: acp::ToolCallStatus) -> acp::ToolCallUpdate {
        acp::ToolCallUpdate::new(
            id.to_string(),
            acp::ToolCallUpdateFields::new().status(status),
        )
    }

    #[test]
    fn request_tracks_tool() {
        let mut statuses = ToolCallStatuses::new();
        statuses.on_tool_call(&make_tool_call(
            "tool-1",
            "Read",
            Some(r#""/path/to/file""#),
        ));
        let lines = statuses.render(&ctx());
        assert_eq!(lines.len(), 1);
        assert!(lines[0].as_str().contains("Read"));
    }

    #[test]
    fn update_to_success() {
        let mut statuses = ToolCallStatuses::new();
        statuses.on_tool_call(&make_tool_call("tool-1", "Read", None));
        statuses.on_tool_call_update(&make_tool_call_update(
            "tool-1",
            acp::ToolCallStatus::Completed,
        ));
        let lines = statuses.render(&ctx());
        assert_eq!(lines.len(), 1);
        assert!(lines[0].as_str().contains("✓"));
    }

    #[test]
    fn unknown_update_is_ignored() {
        let mut statuses = ToolCallStatuses::new();
        statuses.on_tool_call_update(&make_tool_call_update(
            "unknown",
            acp::ToolCallStatus::Completed,
        ));
        let lines = statuses.render(&ctx());
        assert!(lines.is_empty());
    }

    #[test]
    fn update_to_error() {
        let mut statuses = ToolCallStatuses::new();
        statuses.on_tool_call(&make_tool_call("tool-1", "Read", None));
        statuses.on_tool_call_update(&make_tool_call_update(
            "tool-1",
            acp::ToolCallStatus::Failed,
        ));
        let lines = statuses.render(&ctx());
        assert_eq!(lines.len(), 1);
        assert!(lines[0].as_str().contains("X"));
    }

    #[test]
    fn multiple_tools_render_in_order() {
        let mut statuses = ToolCallStatuses::new();
        statuses.on_tool_call(&make_tool_call("tool-1", "Read", None));
        statuses.on_tool_call(&make_tool_call("tool-2", "Write", None));
        let lines = statuses.render(&ctx());
        assert_eq!(lines.len(), 2);
        assert!(lines[0].as_str().contains("Read"));
        assert!(lines[1].as_str().contains("Write"));
    }

    #[test]
    fn multiple_tools_complete_independently() {
        let mut statuses = ToolCallStatuses::new();
        statuses.on_tool_call(&make_tool_call("tool-1", "Read", None));
        statuses.on_tool_call(&make_tool_call("tool-2", "Write", None));
        statuses.on_tool_call_update(&make_tool_call_update(
            "tool-1",
            acp::ToolCallStatus::Completed,
        ));
        let lines = statuses.render(&ctx());
        assert_eq!(lines.len(), 2);
        assert!(lines[0].as_str().contains("✓")); // Read completed
        assert!(!lines[1].as_str().contains("✓")); // Write still running
    }

    #[test]
    fn clear_removes_all() {
        let mut statuses = ToolCallStatuses::new();
        statuses.on_tool_call(&make_tool_call("tool-1", "Read", None));
        statuses.clear();
        assert!(statuses.is_empty());
        assert!(statuses.render(&ctx()).is_empty());
    }

    #[test]
    fn drain_completed_only_removes_success_and_error() {
        let mut statuses = ToolCallStatuses::new();

        statuses.on_tool_call(&make_tool_call("tool-1", "Read", Some(r#""file.rs""#)));
        statuses.on_tool_call(&make_tool_call("tool-2", "Write", Some(r#""out.rs""#)));
        statuses.on_tool_call_update(&make_tool_call_update(
            "tool-2",
            acp::ToolCallStatus::Completed,
        ));
        statuses.on_tool_call(&make_tool_call("tool-3", "Grep", Some(r#""pattern""#)));
        statuses.on_tool_call_update(&make_tool_call_update(
            "tool-3",
            acp::ToolCallStatus::Failed,
        ));

        let drained = statuses.drain_completed(&ctx());
        assert_eq!(drained.len(), 2);
        assert!(drained[0].as_str().contains("Write"));
        assert!(drained[0].as_str().contains("✓"));
        assert!(drained[1].as_str().contains("Grep"));
        assert!(drained[1].as_str().contains("X"));

        let remaining = statuses.render(&ctx());
        assert_eq!(remaining.len(), 1);
        assert!(remaining[0].as_str().contains("Read"));
    }

    #[test]
    fn drain_completed_with_no_completed_returns_empty() {
        let mut statuses = ToolCallStatuses::new();
        statuses.on_tool_call(&make_tool_call("tool-1", "Read", None));
        statuses.on_tool_call(&make_tool_call("tool-2", "Write", None));

        let drained = statuses.drain_completed(&ctx());
        assert!(drained.is_empty());

        let remaining = statuses.render(&ctx());
        assert_eq!(remaining.len(), 2);
    }

    #[test]
    fn drain_completed_allows_late_arriving_results() {
        let mut statuses = ToolCallStatuses::new();

        statuses.on_tool_call(&make_tool_call("tool-1", "Read", None));
        statuses.on_tool_call(&make_tool_call("tool-2", "Write", None));
        statuses.on_tool_call_update(&make_tool_call_update(
            "tool-1",
            acp::ToolCallStatus::Completed,
        ));

        statuses.drain_completed(&ctx());

        statuses.on_tool_call_update(&make_tool_call_update(
            "tool-2",
            acp::ToolCallStatus::Completed,
        ));

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
