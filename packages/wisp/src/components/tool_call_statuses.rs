use acp_utils::notifications::SubAgentProgressParams;
use agent_client_protocol as acp;
use std::collections::HashMap;
use std::time::Instant;

use crate::components::sub_agent_tracker::SubAgentTracker;
use crate::components::tool_call_status_view::{
    ToolCallStatus, render_tool_tree, tool_call_view, compute_diff_preview,
};
use crate::components::tracked_tool_call::{
    TrackedToolCall, raw_input_fragment, upsert_tracked_tool_call,
};
use crate::tui::{Line, ViewContext};

/// Tracks active tool calls and produces status lines for the frame.
#[derive(Clone)]
pub struct ToolCallStatuses {
    /// Ordered list of tool call IDs (insertion order)
    tool_order: Vec<String>,
    /// Tool call info by ID
    tool_calls: HashMap<String, TrackedToolCall>,
    /// Sub-agent states keyed by parent tool call ID
    sub_agents: SubAgentTracker,
    /// Animation tick for the spinner on running tool calls
    tick: u16,
}

pub struct ToolProgress {
    pub running_any: bool,
    pub completed_top_level: usize,
    pub total_top_level: usize,
}

impl ToolCallStatuses {
    pub fn new() -> Self {
        Self {
            tool_order: Vec::new(),
            tool_calls: HashMap::new(),
            sub_agents: SubAgentTracker::default(),
            tick: 0,
        }
    }

    #[cfg(test)]
    pub fn tick(&self) -> u16 {
        self.tick
    }

    pub fn progress(&self) -> ToolProgress {
        let running_any = self.any_running_including_subagents();
        let (completed_top_level, total_top_level) = self.top_level_counts();
        ToolProgress {
            running_any,
            completed_top_level,
            total_top_level,
        }
    }

    /// Advance the animation state. Call this on tick events.
    pub fn on_tick(&mut self, _now: Instant) {
        if self.progress().running_any {
            self.tick = self.tick.wrapping_add(1);
        }
    }

    /// Handle a new tool call from ACP `SessionUpdate::ToolCall`.
    pub fn on_tool_call(&mut self, tool_call: &acp::ToolCall) {
        let id = tool_call.tool_call_id.0.to_string();
        let arguments = tool_call
            .raw_input
            .as_ref()
            .map(raw_input_fragment)
            .unwrap_or_default();

        let tracked = upsert_tracked_tool_call(
            &mut self.tool_order,
            &mut self.tool_calls,
            &id,
            &tool_call.title,
            arguments.clone(),
        );
        tracked.update_name(&tool_call.title);
        tracked.arguments = arguments;
        tracked.status = ToolCallStatus::Running;
    }

    /// Handle a tool call update from ACP `SessionUpdate::ToolCallUpdate`.
    pub fn on_tool_call_update(&mut self, update: &acp::ToolCallUpdate) {
        let id = update.tool_call_id.0.to_string();

        if let Some(tc) = self.tool_calls.get_mut(&id) {
            if let Some(title) = &update.fields.title {
                tc.update_name(title);
            }
            if let Some(raw_input) = &update.fields.raw_input {
                tc.append_arguments(&raw_input_fragment(raw_input));
            }
            if let Some(meta) = &update.meta
                && let Some(dv) = meta.get("display_value").and_then(|v| v.as_str())
            {
                tc.display_value = Some(dv.to_string());
            }
            if let Some(content) = &update.fields.content {
                for item in content {
                    if let acp::ToolCallContent::Diff(diff) = item {
                        tc.diff_preview = Some(compute_diff_preview(diff));
                    }
                }
            }
            if let Some(status) = update.fields.status {
                tc.apply_status(status);
            }
        }
    }

    pub fn has_tool(&self, id: &str) -> bool {
        self.tool_calls.contains_key(id)
    }

    pub fn is_tool_running(&self, id: &str) -> bool {
        self.tool_calls
            .get(id)
            .is_some_and(|tc| matches!(tc.status, ToolCallStatus::Running))
    }

    #[cfg(test)]
    pub fn is_tool_active_for_render(&self, id: &str) -> bool {
        self.is_tool_running(id)
            || self
                .sub_agents
                .get(id)
                .is_some_and(|agents| {
                    agents.iter().any(
                        crate::components::sub_agent_tracker::SubAgentState::is_active_for_render,
                    )
                })
    }

    /// Handle a sub-agent progress notification.
    pub fn on_sub_agent_progress(&mut self, notification: &SubAgentProgressParams) {
        self.sub_agents.on_progress(notification);
    }

    pub fn remove_tool(&mut self, id: &str) {
        self.tool_calls.remove(id);
        self.tool_order.retain(|tool_id| tool_id != id);
        self.sub_agents.remove(id);
    }

    #[cfg(test)]
    pub fn render(&self, context: &ViewContext) -> Vec<Line> {
        self.tool_order
            .iter()
            .flat_map(|id| self.render_tool(id, context))
            .collect()
    }

    pub fn render_tool(&self, id: &str, context: &ViewContext) -> Vec<Line> {
        render_tool_tree(id, &self.tool_calls, &self.sub_agents, self.tick, context)
    }

    /// Clear all tracked tool calls (e.g., after pushing to scrollback).
    pub fn clear(&mut self) {
        self.tool_order.clear();
        self.tool_calls.clear();
        self.sub_agents.clear();
    }

    #[cfg(test)]
    pub fn drain_completed(&mut self, context: &ViewContext) -> Vec<Line> {
        let mut lines = Vec::new();
        let mut completed_ids = Vec::new();

        for id in &self.tool_order {
            if let Some(tc) = self.tool_calls.get(id)
                && !matches!(tc.status, ToolCallStatus::Running)
            {
                let view = tool_call_view(tc, 0);
                lines.extend(view.render(context));
                completed_ids.push(id.clone());
            }
        }

        for id in &completed_ids {
            self.tool_calls.remove(id);
        }
        self.tool_order.retain(|id| !completed_ids.contains(id));

        lines
    }

    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.tool_calls.is_empty()
    }

    fn top_level_counts(&self) -> (usize, usize) {
        let total = self
            .tool_order
            .iter()
            .filter(|id| !self.sub_agents.has_sub_agents(id))
            .count();
        let completed = self
            .tool_order
            .iter()
            .filter(|id| !self.sub_agents.has_sub_agents(id))
            .filter_map(|id| self.tool_calls.get(id))
            .filter(|tc| !matches!(tc.status, ToolCallStatus::Running))
            .count();
        (completed, total)
    }

    fn any_running_including_subagents(&self) -> bool {
        self.tool_calls
            .values()
            .any(|tc| matches!(tc.status, ToolCallStatus::Running))
            || self.sub_agents.any_running()
    }
}

impl Default for ToolCallStatuses {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::tool_call_status_view::{MAX_TOOL_ARG_LENGTH, ToolCallStatusView};
    use crate::tui::{DiffLine, DiffPreview, DiffTag, Line};
    use crate::tui::BRAILLE_FRAMES as FRAMES;
    use acp_utils::notifications::{SubAgentEvent, SubAgentProgressParams};

    fn ctx() -> ViewContext {
        ViewContext::new((80, 24))
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
        assert!(lines[0].plain_text().contains("Read"));
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
        assert!(lines[0].plain_text().contains("✓"));
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
        assert!(lines[0].plain_text().contains("✗"));
    }

    #[test]
    fn multiple_tools_render_in_order() {
        let mut statuses = ToolCallStatuses::new();
        statuses.on_tool_call(&make_tool_call("tool-1", "Read", None));
        statuses.on_tool_call(&make_tool_call("tool-2", "Write", None));
        let lines = statuses.render(&ctx());
        assert_eq!(lines.len(), 2);
        assert!(lines[0].plain_text().contains("Read"));
        assert!(lines[1].plain_text().contains("Write"));
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
        assert!(lines[0].plain_text().contains("✓")); // Read completed
        assert!(!lines[1].plain_text().contains("✓")); // Write still running
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
        assert!(drained[0].plain_text().contains("Write"));
        assert!(drained[0].plain_text().contains("✓"));
        assert!(drained[1].plain_text().contains("Grep"));
        assert!(drained[1].plain_text().contains("✗"));

        let remaining = statuses.render(&ctx());
        assert_eq!(remaining.len(), 1);
        assert!(remaining[0].plain_text().contains("Read"));
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
        assert!(remaining[0].plain_text().contains("Write"));
        assert!(remaining[0].plain_text().contains("✓"));
    }

    #[test]
    fn view_renders_running_with_spinner() {
        let status = ToolCallStatus::Running;
        let view = ToolCallStatusView {
            name: "TestTool",
            arguments: "test args",
            display_value: None,
            diff_preview: None,
            status: &status,
            tick: 0,
        };
        let lines = view.render(&ctx());
        assert_eq!(lines.len(), 1);
        let text = lines[0].plain_text();
        assert!(text.contains("TestTool"));
        assert!(!text.contains("test args"));
        assert!(text.contains(FRAMES[0]));
    }

    #[test]
    fn view_running_spinner_changes_with_tick() {
        let status = ToolCallStatus::Running;
        let view_a = ToolCallStatusView {
            name: "TestTool",
            arguments: "",
            display_value: None,
            diff_preview: None,
            status: &status,
            tick: 0,
        };
        let view_b = ToolCallStatusView {
            name: "TestTool",
            arguments: "",
            display_value: None,
            diff_preview: None,
            status: &status,
            tick: 1,
        };
        let a = view_a.render(&ctx())[0].plain_text();
        let b = view_b.render(&ctx())[0].plain_text();
        assert_ne!(a, b);
    }

    #[test]
    fn view_renders_success() {
        let status = ToolCallStatus::Success;
        let view = ToolCallStatusView {
            name: "TestTool",
            arguments: "test args",
            display_value: None,
            diff_preview: None,
            status: &status,
            tick: 0,
        };
        let lines = view.render(&ctx());
        assert_eq!(lines.len(), 1);
        assert!(lines[0].plain_text().contains("✓"));
    }

    #[test]
    fn view_renders_error() {
        let status = ToolCallStatus::Error("boom".to_string());
        let view = ToolCallStatusView {
            name: "TestTool",
            arguments: "test args",
            display_value: None,
            diff_preview: None,
            status: &status,
            tick: 0,
        };
        let lines = view.render(&ctx());
        assert_eq!(lines.len(), 1);
        assert!(lines[0].plain_text().contains("✗"));
        assert!(lines[0].plain_text().contains("boom"));
    }

    #[test]
    fn view_truncates_utf8_arguments_without_panicking() {
        let arguments = format!("{}界", "a".repeat(MAX_TOOL_ARG_LENGTH - 2));
        let status = ToolCallStatus::Success;
        let view = ToolCallStatusView {
            name: "TestTool",
            arguments: &arguments,
            display_value: None,
            diff_preview: None,
            status: &status,
            tick: 0,
        };

        let lines = view.render(&ctx());
        assert_eq!(lines.len(), 1);
        assert_eq!(
            lines[0].plain_text(),
            format!("✓ TestTool {}", "a".repeat(MAX_TOOL_ARG_LENGTH - 2))
        );
    }

    #[test]
    fn view_running_hides_raw_args_then_shows_display_value() {
        let status = ToolCallStatus::Running;
        let view = ToolCallStatusView {
            name: "Read",
            arguments: r#"{"file_path":"/path/to/main.rs"}"#,
            display_value: None,
            diff_preview: None,
            status: &status,
            tick: 0,
        };

        // While running with no display_value, raw args are hidden
        let text = view.render(&ctx())[0].plain_text();
        assert!(!text.contains("file_path"));
        assert_eq!(text, format!("{} Read", FRAMES[0]));

        // After display_value arrives, it is shown
        let view = ToolCallStatusView {
            display_value: Some("main.rs"),
            ..view
        };
        let text = view.render(&ctx())[0].plain_text();
        assert_eq!(text, format!("{} Read (main.rs)", FRAMES[0]));
    }

    fn make_sub_agent_notification(
        parent_tool_id: &str,
        agent_name: &str,
        event_json: &str,
    ) -> SubAgentProgressParams {
        make_sub_agent_notification_with_task_id(parent_tool_id, agent_name, agent_name, event_json)
    }

    fn make_sub_agent_notification_with_task_id(
        parent_tool_id: &str,
        task_id: &str,
        agent_name: &str,
        event_json: &str,
    ) -> SubAgentProgressParams {
        let json = format!(
            r#"{{"parent_tool_id":"{parent_tool_id}","task_id":"{task_id}","agent_name":"{agent_name}","event":{event_json}}}"#,
        );
        serde_json::from_str(&json).unwrap()
    }

    #[test]
    fn deserialize_tool_call_event() {
        let n = make_sub_agent_notification(
            "p1",
            "explorer",
            r#"{"ToolCall":{"request":{"id":"c1","name":"grep","arguments":"{\"pattern\":\"test\"}"},"model_name":"m"}}"#,
        );
        assert!(matches!(n.event, SubAgentEvent::ToolCall { .. }));
    }

    #[test]
    fn deserialize_tool_call_update_event() {
        let n = make_sub_agent_notification(
            "p1",
            "explorer",
            r#"{"ToolCallUpdate":{"update":{"id":"c1","chunk":"{\"pattern\":\"updated\"}"},"model_name":"m"}}"#,
        );
        assert!(matches!(n.event, SubAgentEvent::ToolCallUpdate { .. }));
    }

    #[test]
    fn deserialize_tool_result_event() {
        let n = make_sub_agent_notification(
            "p1",
            "explorer",
            r#"{"ToolResult":{"result":{"id":"c1","name":"grep","arguments":"{}","result":"ok"},"model_name":"m"}}"#,
        );
        assert!(matches!(n.event, SubAgentEvent::ToolResult { .. }));
    }

    #[test]
    fn deserialize_done_event() {
        let n = make_sub_agent_notification("p1", "explorer", r#""Done""#);
        assert!(matches!(n.event, SubAgentEvent::Done));
    }

    #[test]
    fn deserialize_other_variant() {
        let n = make_sub_agent_notification("p1", "explorer", r#""Other""#);
        assert!(matches!(n.event, SubAgentEvent::Other));
    }

    #[test]
    fn sub_agent_tool_call_renders_nested() {
        let mut statuses = ToolCallStatuses::new();
        statuses.on_tool_call(&make_tool_call("parent-1", "spawn_subagent", None));

        statuses.on_sub_agent_progress(&make_sub_agent_notification(
            "parent-1",
            "explorer",
            r#"{"ToolCall":{"request":{"id":"c1","name":"grep","arguments":"{\"pattern\":\"test\"}"},"model_name":"m"}}"#,
        ));

        let lines = statuses.render_tool("parent-1", &ctx());
        assert_eq!(lines.len(), 2);
        assert!(lines[0].plain_text().contains("explorer"));
        assert!(lines[0].plain_text().starts_with("  "));
        assert!(lines[1].plain_text().starts_with("  └─ "));
        assert!(lines[1].plain_text().contains("grep"));
    }

    #[test]
    fn sub_agent_tool_call_update_appends_chunk() {
        let mut statuses = ToolCallStatuses::new();
        statuses.on_tool_call(&make_tool_call("parent-1", "spawn_subagent", None));

        statuses.on_sub_agent_progress(&make_sub_agent_notification(
            "parent-1",
            "explorer",
            r#"{"ToolCall":{"request":{"id":"c1","name":"grep","arguments":""},"model_name":"m"}}"#,
        ));
        statuses.on_sub_agent_progress(&make_sub_agent_notification(
            "parent-1",
            "explorer",
            r#"{"ToolCallUpdate":{"update":{"id":"c1","chunk":"{\"pattern\":\"updated\"}"}}}"#,
        ));
        statuses.on_sub_agent_progress(&make_sub_agent_notification(
            "parent-1",
            "explorer",
            r#"{"ToolResult":{"result":{"id":"c1","name":"grep","arguments":"{}","result":"ok"},"model_name":"m"}}"#,
        ));

        let lines = statuses.render_tool("parent-1", &ctx());
        assert_eq!(lines.len(), 2);
        assert!(lines[1].plain_text().contains("updated"));
    }

    #[test]
    fn sub_agent_tool_result_shows_checkmark() {
        let mut statuses = ToolCallStatuses::new();
        statuses.on_tool_call(&make_tool_call("parent-1", "spawn_subagent", None));

        statuses.on_sub_agent_progress(&make_sub_agent_notification(
            "parent-1",
            "explorer",
            r#"{"ToolCall":{"request":{"id":"c1","name":"read_file","arguments":"{}"},"model_name":"m"}}"#,
        ));
        statuses.on_sub_agent_progress(&make_sub_agent_notification(
            "parent-1",
            "explorer",
            r#"{"ToolResult":{"result":{"id":"c1","name":"read_file","arguments":"{}","result":"ok"},"model_name":"m"}}"#,
        ));

        let lines = statuses.render_tool("parent-1", &ctx());
        assert_eq!(lines.len(), 2);
        assert!(lines[1].plain_text().contains("✓"));
    }

    #[test]
    fn sub_agent_tool_result_uses_result_meta() {
        let mut statuses = ToolCallStatuses::new();
        statuses.on_tool_call(&make_tool_call("parent-1", "spawn_subagent", None));

        statuses.on_sub_agent_progress(&make_sub_agent_notification(
            "parent-1",
            "explorer",
            r#"{"ToolCall":{"request":{"id":"c1","name":"coding__read_file","arguments":"{\"filePath\":\"Cargo.toml\"}"},"model_name":"m"}}"#,
        ));
        statuses.on_sub_agent_progress(&make_sub_agent_notification(
            "parent-1",
            "explorer",
            r#"{"ToolResult":{"result":{"id":"c1","name":"coding__read_file","result_meta":{"display":{"title":"Read file","value":"Cargo.toml, 156 lines"}}},"model_name":"m"}}"#,
        ));

        let lines = statuses.render_tool("parent-1", &ctx());
        assert_eq!(lines.len(), 2);
        let tool_line = lines[1].plain_text();
        assert!(tool_line.contains("✓"));
        assert!(tool_line.contains("Read file"));
        assert!(tool_line.contains("(Cargo.toml, 156 lines)"));
        assert!(!tool_line.contains("filePath"));
    }

    #[test]
    fn sub_agent_tool_error_shows_x() {
        let mut statuses = ToolCallStatuses::new();
        statuses.on_tool_call(&make_tool_call("parent-1", "spawn_subagent", None));

        statuses.on_sub_agent_progress(&make_sub_agent_notification(
            "parent-1",
            "explorer",
            r#"{"ToolCall":{"request":{"id":"c1","name":"read_file","arguments":"{}"},"model_name":"m"}}"#,
        ));
        statuses.on_sub_agent_progress(&make_sub_agent_notification(
            "parent-1",
            "explorer",
            r#"{"ToolError":{"error":{"id":"c1","name":"read_file","arguments":"{}","error":"not found"},"model_name":"m"}}"#,
        ));

        let lines = statuses.render_tool("parent-1", &ctx());
        assert_eq!(lines.len(), 2);
        assert!(lines[1].plain_text().contains("✗"));
    }

    #[test]
    fn multiple_sub_agents_render_separate_headers() {
        let mut statuses = ToolCallStatuses::new();
        statuses.on_tool_call(&make_tool_call("parent-1", "spawn_subagent", None));

        statuses.on_sub_agent_progress(&make_sub_agent_notification(
            "parent-1",
            "explorer",
            r#"{"ToolCall":{"request":{"id":"c1","name":"grep","arguments":"{}"},"model_name":"m"}}"#,
        ));
        statuses.on_sub_agent_progress(&make_sub_agent_notification(
            "parent-1",
            "writer",
            r#"{"ToolCall":{"request":{"id":"c2","name":"write_file","arguments":"{}"},"model_name":"m"}}"#,
        ));

        let lines = statuses.render_tool("parent-1", &ctx());
        assert_eq!(lines.len(), 5);
        assert!(lines[0].plain_text().contains("explorer"));
        assert!(lines[3].plain_text().contains("writer"));
    }

    #[test]
    fn same_name_agents_with_different_task_ids_render_separately() {
        let mut statuses = ToolCallStatuses::new();
        statuses.on_tool_call(&make_tool_call("parent-1", "spawn_subagent", None));

        statuses.on_sub_agent_progress(&make_sub_agent_notification_with_task_id(
            "parent-1",
            "task-1",
            "codebase-explorer",
            r#"{"ToolCall":{"request":{"id":"c1","name":"grep","arguments":"{}"},"model_name":"m"}}"#,
        ));
        statuses.on_sub_agent_progress(&make_sub_agent_notification_with_task_id(
            "parent-1",
            "task-2",
            "codebase-explorer",
            r#"{"ToolCall":{"request":{"id":"c2","name":"read_file","arguments":"{}"},"model_name":"m"}}"#,
        ));
        statuses.on_sub_agent_progress(&make_sub_agent_notification_with_task_id(
            "parent-1",
            "task-3",
            "codebase-explorer",
            r#"{"ToolCall":{"request":{"id":"c3","name":"list_files","arguments":"{}"},"model_name":"m"}}"#,
        ));

        let lines = statuses.render_tool("parent-1", &ctx());
        assert_eq!(lines.len(), 8);
        assert!(lines[1].plain_text().contains("grep"));
        assert!(lines[4].plain_text().contains("read_file"));
        assert!(lines[7].plain_text().contains("list_files"));
    }

    #[test]
    fn sub_agent_renders_latest_three_tools_with_overflow() {
        let mut statuses = ToolCallStatuses::new();
        statuses.on_tool_call(&make_tool_call("parent-1", "spawn_subagent", None));

        // Tool 1
        statuses.on_sub_agent_progress(&make_sub_agent_notification(
            "parent-1",
            "explorer",
            r#"{"ToolCall":{"request":{"id":"c1","name":"grep","arguments":"{}"},"model_name":"m"}}"#,
        ));
        statuses.on_sub_agent_progress(&make_sub_agent_notification(
            "parent-1",
            "explorer",
            r#"{"ToolResult":{"result":{"id":"c1","name":"grep","arguments":"{}","result":"ok"},"model_name":"m"}}"#,
        ));
        // Tool 2
        statuses.on_sub_agent_progress(&make_sub_agent_notification(
            "parent-1",
            "explorer",
            r#"{"ToolCall":{"request":{"id":"c2","name":"read_file","arguments":"{}"},"model_name":"m"}}"#,
        ));
        // Tool 3
        statuses.on_sub_agent_progress(&make_sub_agent_notification(
            "parent-1",
            "explorer",
            r#"{"ToolCall":{"request":{"id":"c3","name":"list_files","arguments":"{}"},"model_name":"m"}}"#,
        ));
        // Tool 4
        statuses.on_sub_agent_progress(&make_sub_agent_notification(
            "parent-1",
            "explorer",
            r#"{"ToolCall":{"request":{"id":"c4","name":"write_file","arguments":"{}"},"model_name":"m"}}"#,
        ));

        let lines = statuses.render_tool("parent-1", &ctx());
        assert_eq!(lines.len(), 5);
        assert!(lines[1].plain_text().contains("1 earlier tool calls"));
        assert!(lines[2].plain_text().contains("read_file"));
        assert!(lines[2].plain_text().contains("├─"));
        assert!(lines[3].plain_text().contains("list_files"));
        assert!(lines[3].plain_text().contains("├─"));
        assert!(lines[4].plain_text().contains("write_file"));
        assert!(lines[4].plain_text().contains("└─"));
    }

    #[test]
    fn progress_reports_sub_agent_running_tools() {
        let mut statuses = ToolCallStatuses::new();
        statuses.on_tool_call(&make_tool_call("parent-1", "spawn_subagent", None));
        statuses.on_tool_call_update(&make_tool_call_update(
            "parent-1",
            acp::ToolCallStatus::Completed,
        ));
        statuses.on_sub_agent_progress(&make_sub_agent_notification(
            "parent-1",
            "explorer",
            r#"{"ToolCall":{"request":{"id":"c1","name":"grep","arguments":"{}"},"model_name":"m"}}"#,
        ));

        assert!(statuses.progress().running_any);
    }

    #[test]
    fn remove_tool_cleans_up_sub_agent_state() {
        let mut statuses = ToolCallStatuses::new();
        statuses.on_tool_call(&make_tool_call("parent-1", "spawn_subagent", None));
        statuses.on_sub_agent_progress(&make_sub_agent_notification(
            "parent-1",
            "explorer",
            r#"{"ToolCall":{"request":{"id":"c1","name":"grep","arguments":"{}"},"model_name":"m"}}"#,
        ));

        statuses.remove_tool("parent-1");
        assert!(!statuses.progress().running_any);
        assert!(statuses.render_tool("parent-1", &ctx()).is_empty());
    }

    #[test]
    fn clear_removes_sub_agent_state() {
        let mut statuses = ToolCallStatuses::new();
        statuses.on_tool_call(&make_tool_call("parent-1", "spawn_subagent", None));
        statuses.on_sub_agent_progress(&make_sub_agent_notification(
            "parent-1",
            "explorer",
            r#"{"ToolCall":{"request":{"id":"c1","name":"grep","arguments":"{}"},"model_name":"m"}}"#,
        ));

        statuses.clear();
        assert!(!statuses.progress().running_any);
    }

    #[test]
    fn agent_header_shows_spinner_while_running() {
        let mut statuses = ToolCallStatuses::new();
        statuses.on_tool_call(&make_tool_call("parent-1", "spawn_subagent", None));

        statuses.on_sub_agent_progress(&make_sub_agent_notification(
            "parent-1",
            "explorer",
            r#"{"ToolCall":{"request":{"id":"c1","name":"grep","arguments":"{}"},"model_name":"m"}}"#,
        ));
        statuses.on_sub_agent_progress(&make_sub_agent_notification(
            "parent-1",
            "explorer",
            r#"{"ToolResult":{"result":{"id":"c1","name":"grep","arguments":"{}","result":"ok"},"model_name":"m"}}"#,
        ));

        let lines = statuses.render_tool("parent-1", &ctx());
        let header = lines[0].plain_text();
        assert!(
            !header.contains('✓'),
            "Expected spinner, not ✓ in header: {header}"
        );
    }

    #[test]
    fn agent_header_shows_done_after_done_event() {
        let mut statuses = ToolCallStatuses::new();
        statuses.on_tool_call(&make_tool_call("parent-1", "spawn_subagent", None));

        statuses.on_sub_agent_progress(&make_sub_agent_notification(
            "parent-1",
            "explorer",
            r#"{"ToolCall":{"request":{"id":"c1","name":"grep","arguments":"{}"},"model_name":"m"}}"#,
        ));
        statuses.on_sub_agent_progress(&make_sub_agent_notification(
            "parent-1",
            "explorer",
            r#""Done""#,
        ));

        let lines = statuses.render_tool("parent-1", &ctx());
        let header = lines[0].plain_text();
        assert!(header.contains('✓'), "Expected ✓ in header: {header}");
    }

    #[test]
    fn test_display_value_shown_on_completion() {
        let mut statuses = ToolCallStatuses::new();
        statuses.on_tool_call(&make_tool_call("tool-1", "coding__read_file", None));

        let mut meta_map = serde_json::Map::new();
        meta_map.insert("display_value".into(), "Cargo.toml, 156 lines".into());
        let update = acp::ToolCallUpdate::new(
            "tool-1".to_string(),
            acp::ToolCallUpdateFields::new()
                .title("Read file")
                .status(acp::ToolCallStatus::Completed),
        )
        .meta(meta_map);
        statuses.on_tool_call_update(&update);

        let lines = statuses.render(&ctx());
        assert_eq!(lines.len(), 1);
        let text = lines[0].plain_text();
        assert!(
            text.contains("Read file"),
            "Expected display title in output: {text}"
        );
        assert!(
            text.contains("(Cargo.toml, 156 lines)"),
            "Expected display value in output: {text}"
        );
    }

    #[test]
    fn test_display_value_shown_while_running() {
        let mut statuses = ToolCallStatuses::new();
        statuses.on_tool_call(&make_tool_call(
            "tool-1",
            "Read file",
            Some(r#"{"file_path":"/path/to/main.rs"}"#),
        ));

        let mut meta_map = serde_json::Map::new();
        meta_map.insert("display_value".into(), "main.rs".into());
        let update = acp::ToolCallUpdate::new(
            "tool-1".to_string(),
            acp::ToolCallUpdateFields::new(),
        )
        .meta(meta_map);
        statuses.on_tool_call_update(&update);

        let lines = statuses.render(&ctx());
        assert_eq!(lines.len(), 1);
        let text = lines[0].plain_text();
        assert!(
            text.contains("(main.rs)"),
            "Expected display value while running: {text}"
        );
        assert!(
            !text.contains("file_path"),
            "Raw args should not appear: {text}"
        );
    }

    #[test]
    fn test_diff_preview_rendered_on_success() {
        let mut statuses = ToolCallStatuses::new();
        statuses.on_tool_call(&make_tool_call("tool-1", "Edit", None));

        let tc = statuses.tool_calls.get_mut("tool-1").unwrap();
        tc.status = ToolCallStatus::Success;
        tc.diff_preview = Some(DiffPreview {
            lines: vec![
                DiffLine {
                    tag: DiffTag::Removed,
                    content: "old line".to_string(),
                },
                DiffLine {
                    tag: DiffTag::Added,
                    content: "new line".to_string(),
                },
            ],
            lang_hint: "rs".to_string(),
            start_line: Some(1),
        });

        let lines = statuses.render(&ctx());
        assert!(lines.len() > 1);
        let all_text: String = lines.iter().map(|l| l.plain_text()).collect();
        assert!(
            all_text.contains("old line"),
            "Expected removed line: {all_text}"
        );
        assert!(
            all_text.contains("new line"),
            "Expected added line: {all_text}"
        );
    }

    #[test]
    fn test_diff_preview_not_rendered_while_running() {
        let mut statuses = ToolCallStatuses::new();
        statuses.on_tool_call(&make_tool_call("tool-1", "Edit", None));

        let tc = statuses.tool_calls.get_mut("tool-1").unwrap();
        tc.diff_preview = Some(DiffPreview {
            lines: vec![DiffLine {
                tag: DiffTag::Added,
                content: "new line".to_string(),
            }],
            lang_hint: "rs".to_string(),
            start_line: Some(1),
        });

        let lines = statuses.render(&ctx());
        assert_eq!(lines.len(), 1, "Should only have status line while running");
    }
}
