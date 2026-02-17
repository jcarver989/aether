use agent_client_protocol as acp;

use crate::tui::spinner::BRAILLE_FRAMES as FRAMES;
use crate::tui::{Component, Line, RenderContext};
use crossterm::style::Stylize;
use std::collections::HashMap;

const MAX_TOOL_ARG_LENGTH: usize = 200;

/// Renders a single tool call status line.
pub struct ToolCallStatusView {
    pub name: String,
    pub arguments: String,
    pub status: ToolCallStatus,
    pub tick: u16,
}

pub enum ToolCallStatus {
    Running,
    Success,
    Error(String),
}

impl Component for ToolCallStatusView {
    fn render(&self, context: &RenderContext) -> Vec<Line> {
        // Only color the indicator/spinner, not the tool name
        let (indicator, suffix) = match &self.status {
            ToolCallStatus::Running => {
                let frame = FRAMES[self.tick as usize % FRAMES.len()];
                (frame.with(context.theme.info), String::new())
            }
            ToolCallStatus::Success => (
                '●'.with(context.theme.success),
                " ✓".with(context.theme.success).to_string(),
            ),
            ToolCallStatus::Error(_) => (
                '●'.with(context.theme.error),
                " X".with(context.theme.error).to_string(),
            ),
        };

        // Tool name in default/white color (uncolored)
        let name_styled = format!("{} {}", indicator, self.name);
        let args = Self::format_arguments(&self.arguments, context);

        let mut line_text = format!("{name_styled}{suffix}{args}");

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
    /// Animation tick for the spinner on running tool calls
    tick: u16,
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
            tick: 0,
        }
    }

    /// Update the animation tick for running tool call spinners.
    pub fn set_tick(&mut self, tick: u16) {
        self.tick = tick;
    }

    /// Returns true if any tool calls are still running.
    pub fn has_running(&self) -> bool {
        self.tool_calls
            .values()
            .any(|tc| matches!(tc.status, TrackedStatus::Running))
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

    pub fn has_tool(&self, id: &str) -> bool {
        self.tool_calls.contains_key(id)
    }

    pub fn is_tool_running(&self, id: &str) -> bool {
        self.tool_calls
            .get(id)
            .map(|tc| matches!(tc.status, TrackedStatus::Running))
            .unwrap_or(false)
    }

    pub fn remove_tool(&mut self, id: &str) {
        self.tool_calls.remove(id);
        self.tool_order.retain(|tool_id| tool_id != id);
    }

    pub fn render_tool(&self, id: &str, context: &RenderContext) -> Vec<Line> {
        self.view_for(id, self.tick)
            .map(|view| view.render(context))
            .unwrap_or_default()
    }

    /// Clear all tracked tool calls (e.g., after pushing to scrollback).
    #[allow(dead_code)]
    pub fn clear(&mut self) {
        self.tool_order.clear();
        self.tool_calls.clear();
    }

    /// Render and remove only completed (Success/Error) tool calls,
    /// leaving Running ones in place for continued display.
    #[allow(dead_code)]
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
                            tick: 0,
                        };
                        lines.extend(view.render(context));
                        completed_ids.push(id.clone());
                    }
                    TrackedStatus::Error(msg) => {
                        let view = ToolCallStatusView {
                            name: tc.name.clone(),
                            arguments: tc.arguments.clone(),
                            status: ToolCallStatus::Error(msg.clone()),
                            tick: 0,
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

    fn view_for(&self, id: &str, tick: u16) -> Option<ToolCallStatusView> {
        let tc = self.tool_calls.get(id)?;
        let status = match &tc.status {
            TrackedStatus::Running => ToolCallStatus::Running,
            TrackedStatus::Success => ToolCallStatus::Success,
            TrackedStatus::Error(msg) => ToolCallStatus::Error(msg.clone()),
        };

        Some(ToolCallStatusView {
            name: tc.name.clone(),
            arguments: tc.arguments.clone(),
            status,
            tick,
        })
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
            if let Some(view) = self.view_for(id, self.tick) {
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
    fn view_renders_running_with_spinner() {
        let view = ToolCallStatusView {
            name: "TestTool".to_string(),
            arguments: "test args".to_string(),
            status: ToolCallStatus::Running,
            tick: 0,
        };
        let lines = view.render(&ctx());
        assert_eq!(lines.len(), 1);
        let text = lines[0].as_str();
        assert!(text.contains("TestTool"));
        assert!(text.contains("test args"));
        assert!(text.contains('⠋'));
    }

    #[test]
    fn view_running_spinner_changes_with_tick() {
        let view_a = ToolCallStatusView {
            name: "TestTool".to_string(),
            arguments: "".to_string(),
            status: ToolCallStatus::Running,
            tick: 0,
        };
        let view_b = ToolCallStatusView {
            name: "TestTool".to_string(),
            arguments: "".to_string(),
            status: ToolCallStatus::Running,
            tick: 1,
        };
        let a = view_a.render(&ctx())[0].as_str().to_string();
        let b = view_b.render(&ctx())[0].as_str().to_string();
        assert_ne!(a, b);
    }

    #[test]
    fn view_renders_success() {
        let view = ToolCallStatusView {
            name: "TestTool".to_string(),
            arguments: "test args".to_string(),
            status: ToolCallStatus::Success,
            tick: 0,
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
            tick: 0,
        };
        let lines = view.render(&ctx());
        assert_eq!(lines.len(), 1);
        assert!(lines[0].as_str().contains("X"));
        assert!(lines[0].as_str().contains("boom"));
    }
}
