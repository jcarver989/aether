use acp_utils::notifications::{SubAgentEvent, SubAgentProgressParams, ToolResultMeta};
use agent_client_protocol as acp;
use std::collections::HashMap;
use std::time::Instant;

use crate::components::tool_call_status_view::{
    ToolCallStatus, ToolCallStatusView, compute_diff_preview,
};
use crate::tui::BRAILLE_FRAMES as FRAMES;
use crate::tui::{DiffPreview, Line, ViewContext};

const SUB_AGENT_VISIBLE_TOOL_LIMIT: usize = 3;

/// Per-sub-agent state: tracks its tool calls in order.
#[derive(Clone)]
struct SubAgentState {
    task_id: String,
    agent_name: String,
    done: bool,
    tool_order: Vec<String>,
    tool_calls: HashMap<String, TrackedToolCall>,
}

/// Tracks active tool calls and produces status lines for the frame.
#[derive(Clone)]
pub struct ToolCallStatuses {
    /// Ordered list of tool call IDs (insertion order)
    tool_order: Vec<String>,
    /// Tool call info by ID
    tool_calls: HashMap<String, TrackedToolCall>,
    /// Sub-agent tool states keyed by parent tool call ID.
    /// Each parent can have multiple sub-agents (tracked in insertion order).
    sub_agents: HashMap<String, Vec<SubAgentState>>,
    /// Animation tick for the spinner on running tool calls
    tick: u16,
}

pub struct ToolProgress {
    pub running_any: bool,
    pub completed_top_level: usize,
    pub total_top_level: usize,
}

#[derive(Clone)]
struct TrackedToolCall {
    name: String,
    arguments: String,
    display_value: Option<String>,
    diff_preview: Option<DiffPreview>,
    status: ToolCallStatus,
}

impl TrackedToolCall {
    fn new_running(name: impl Into<String>, arguments: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            arguments: arguments.into(),
            display_value: None,
            diff_preview: None,
            status: ToolCallStatus::Running,
        }
    }

    fn update_name(&mut self, name: &str) {
        if !name.is_empty() {
            self.name.clear();
            self.name.push_str(name);
        }
    }

    fn append_arguments(&mut self, fragment: &str) {
        self.arguments.push_str(fragment);
    }

    fn apply_result_meta(&mut self, meta: ToolResultMeta) {
        self.name.clone_from(&meta.display.title);
        self.display_value = Some(meta.display.value);
    }

    fn apply_status(&mut self, status: acp::ToolCallStatus) {
        match status {
            acp::ToolCallStatus::Completed => self.status = ToolCallStatus::Success,
            acp::ToolCallStatus::Failed => {
                self.status = ToolCallStatus::Error("failed".to_string());
            }
            acp::ToolCallStatus::InProgress | acp::ToolCallStatus::Pending => {
                self.status = ToolCallStatus::Running;
            }
            _ => {}
        }
    }
}

fn raw_input_fragment(raw_input: &serde_json::Value) -> String {
    raw_input
        .as_str()
        .map_or_else(|| raw_input.to_string(), str::to_string)
}

fn upsert_tracked_tool_call<'a>(
    tool_order: &mut Vec<String>,
    tool_calls: &'a mut HashMap<String, TrackedToolCall>,
    id: &str,
    default_name: &str,
    default_arguments: String,
) -> &'a mut TrackedToolCall {
    if !tool_calls.contains_key(id) {
        tool_order.push(id.to_string());
    }

    tool_calls
        .entry(id.to_string())
        .or_insert_with(|| TrackedToolCall::new_running(default_name, default_arguments))
}

impl ToolCallStatuses {
    pub fn new() -> Self {
        Self {
            tool_order: Vec::new(),
            tool_calls: HashMap::new(),
            sub_agents: HashMap::new(),
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

    fn top_level_counts(&self) -> (usize, usize) {
        let total = self
            .tool_order
            .iter()
            .filter(|id| !self.has_sub_agents(id))
            .count();
        let completed = self
            .tool_order
            .iter()
            .filter(|id| !self.has_sub_agents(id))
            .filter_map(|id| self.tool_calls.get(id))
            .filter(|tc| !matches!(tc.status, ToolCallStatus::Running))
            .count();
        (completed, total)
    }

    fn any_running_including_subagents(&self) -> bool {
        self.tool_calls
            .values()
            .any(|tc| matches!(tc.status, ToolCallStatus::Running))
            || self
                .sub_agents
                .values()
                .any(|agents| agents.iter().any(SubAgentState::is_active_for_render))
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
                .is_some_and(|agents| agents.iter().any(SubAgentState::is_active_for_render))
    }

    /// Handle a sub-agent progress notification.
    pub fn on_sub_agent_progress(&mut self, notification: &SubAgentProgressParams) {
        let agents = self
            .sub_agents
            .entry(notification.parent_tool_id.clone())
            .or_default();

        let agent = if let Some(a) = agents
            .iter_mut()
            .find(|a| a.task_id == notification.task_id)
        {
            a
        } else {
            agents.push(SubAgentState {
                task_id: notification.task_id.clone(),
                agent_name: notification.agent_name.clone(),
                done: false,
                tool_order: Vec::new(),
                tool_calls: HashMap::new(),
            });
            agents.last_mut().unwrap()
        };

        match &notification.event {
            SubAgentEvent::ToolCall { request } => {
                let tracked = upsert_tracked_tool_call(
                    &mut agent.tool_order,
                    &mut agent.tool_calls,
                    &request.id,
                    &request.name,
                    request.arguments.clone(),
                );
                tracked.update_name(&request.name);
                tracked.arguments.clone_from(&request.arguments);
                tracked.status = ToolCallStatus::Running;
            }
            SubAgentEvent::ToolCallUpdate { update } => {
                let tracked = upsert_tracked_tool_call(
                    &mut agent.tool_order,
                    &mut agent.tool_calls,
                    &update.id,
                    "tool",
                    String::new(),
                );
                tracked.append_arguments(&update.chunk);
                tracked.status = ToolCallStatus::Running;
            }
            SubAgentEvent::ToolResult { result } => {
                if let Some(tc) = agent.tool_calls.get_mut(&result.id) {
                    tc.status = ToolCallStatus::Success;
                    if let Some(result_meta) = &result.result_meta {
                        tc.apply_result_meta(result_meta.clone());
                    }
                }
            }
            SubAgentEvent::ToolError { error } => {
                if let Some(tc) = agent.tool_calls.get_mut(&error.id) {
                    tc.status = ToolCallStatus::Error("failed".to_string());
                }
            }
            SubAgentEvent::Done => {
                agent.done = true;
            }
            SubAgentEvent::Other => {}
        }
    }

    fn has_sub_agents(&self, tool_id: &str) -> bool {
        self.sub_agents.get(tool_id).is_some_and(|a| !a.is_empty())
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
        let has_sub_agents = self.has_sub_agents(id);

        let mut lines = if has_sub_agents {
            Vec::new()
        } else {
            self.view_for(id, self.tick)
                .map(|view| view.render(context))
                .unwrap_or_default()
        };

        if let Some(agents) = self.sub_agents.get(id) {
            for (i, agent) in agents.iter().enumerate() {
                // Blank line between multiple agents, but not before the first
                if i > 0 {
                    lines.push(Line::default());
                }
                lines.push(self.render_agent_header(agent, context));

                let hidden_count = agent
                    .tool_order
                    .len()
                    .saturating_sub(SUB_AGENT_VISIBLE_TOOL_LIMIT);

                if hidden_count > 0 {
                    let mut summary = Line::default();
                    summary.push_styled(
                        format!("  … {hidden_count} earlier tool calls"),
                        context.theme.muted(),
                    );
                    lines.push(summary);
                }

                let mut visible = agent
                    .tool_order
                    .iter()
                    .skip(hidden_count)
                    .filter_map(|tool_id| agent.tool_calls.get(tool_id))
                    .peekable();

                while let Some(tc) = visible.next() {
                    let connector = if visible.peek().is_some() {
                        "  ├─ "
                    } else {
                        "  └─ "
                    };

                    let view = Self::tool_call_view(tc, self.tick);
                    for tool_line in view.render(context) {
                        let mut indented = Line::default();
                        indented.push_styled(connector, context.theme.muted());
                        for span in tool_line.spans() {
                            indented.push_with_style(span.text(), span.style());
                        }
                        lines.push(indented);
                    }
                }
            }
        }

        lines
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
                let view = Self::tool_call_view(tc, 0);
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

    fn render_agent_header(&self, agent: &SubAgentState, context: &ViewContext) -> Line {
        let mut line = Line::default();
        line.push_text("  ");
        if agent.done {
            line.push_styled("✓".to_string(), context.theme.success());
        } else {
            let frame = FRAMES[self.tick as usize % FRAMES.len()];
            line.push_styled(frame.to_string(), context.theme.info());
        }
        line.push_text(" ");
        line.push_text(&agent.agent_name);
        line
    }

    fn tool_call_view(tc: &TrackedToolCall, tick: u16) -> ToolCallStatusView<'_> {
        ToolCallStatusView {
            name: &tc.name,
            arguments: &tc.arguments,
            display_value: tc.display_value.as_deref(),
            diff_preview: tc.diff_preview.as_ref(),
            status: &tc.status,
            tick,
        }
    }

    fn view_for(&self, id: &str, tick: u16) -> Option<ToolCallStatusView<'_>> {
        self.tool_calls
            .get(id)
            .map(|tc| Self::tool_call_view(tc, tick))
    }
}

impl Default for ToolCallStatuses {
    fn default() -> Self {
        Self::new()
    }
}

impl SubAgentState {
    fn is_active_for_render(&self) -> bool {
        !self.done
            || self
                .tool_calls
                .values()
                .any(|tc| matches!(tc.status, ToolCallStatus::Running))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::tool_call_status_view::MAX_TOOL_ARG_LENGTH;
    use crate::tui::{DiffLine, DiffTag, Line};

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

    // -- Sub-agent deserialization tests --

    fn make_sub_agent_notification(
        parent_tool_id: &str,
        agent_name: &str,
        event_json: &str,
    ) -> SubAgentProgressParams {
        // Use agent_name as task_id for convenience; override with
        // make_sub_agent_notification_with_task_id when testing multiple
        // agents with the same name.
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
        // "model_name" is present because the wire format comes from AgentMessage serialization;
        // SubAgentEvent::ToolCallUpdate has no model_name field, so serde silently ignores it.
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

    // -- Sub-agent tracking tests --

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
        // Line 0: agent header, Line 1: sub-tool (parent line hidden)
        assert_eq!(lines.len(), 2);
        assert!(lines[0].plain_text().contains("explorer"));
        assert!(lines[0].plain_text().starts_with("  ")); // 2-space indent
        assert!(lines[1].plain_text().starts_with("  └─ ")); // tree connector
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
        // agent header + last tool (completed); parent line hidden
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
        // agent header + last tool (errored); parent line hidden
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
        // explorer header + explorer tool + blank + writer header + writer tool
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
        // (header + tool) + 2 * (blank + header + tool) = 8 lines
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
        // agent header + overflow summary + latest 3 tools (parent line hidden)
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
        // Parent is done, but sub-agent tool is still running
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

        // Tool completed but agent hasn't sent Done yet
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
        // Agent is still running (no Done event), so header should show spinner
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
            Some(r#"{"filePath":"Cargo.toml"}"#),
        ));

        let mut meta_map = serde_json::Map::new();
        meta_map.insert("display_value".into(), "Cargo.toml".into());
        let update = acp::ToolCallUpdate::new(
            "tool-1".to_string(),
            acp::ToolCallUpdateFields::new()
                .title("Read file")
                .status(acp::ToolCallStatus::InProgress),
        )
        .meta(meta_map);
        statuses.on_tool_call_update(&update);

        let lines = statuses.render(&ctx());
        assert_eq!(lines.len(), 1);
        let text = lines[0].plain_text();
        assert!(
            text.contains("(Cargo.toml)"),
            "Preview display_value should appear while running: {text}"
        );
    }

    #[test]
    fn test_result_meta_overrides_preview() {
        let mut statuses = ToolCallStatuses::new();
        statuses.on_tool_call(&make_tool_call(
            "tool-1",
            "Read file",
            Some(r#"{"filePath":"Cargo.toml"}"#),
        ));

        let mut preview_map = serde_json::Map::new();
        preview_map.insert("display_value".into(), "Cargo.toml".into());
        let update = acp::ToolCallUpdate::new(
            "tool-1".to_string(),
            acp::ToolCallUpdateFields::new()
                .title("Read file")
                .status(acp::ToolCallStatus::InProgress),
        )
        .meta(preview_map);
        statuses.on_tool_call_update(&update);

        let mut result_map = serde_json::Map::new();
        result_map.insert("display_value".into(), "Cargo.toml, 156 lines".into());
        let update = acp::ToolCallUpdate::new(
            "tool-1".to_string(),
            acp::ToolCallUpdateFields::new()
                .title("Read file")
                .status(acp::ToolCallStatus::Completed),
        )
        .meta(result_map);
        statuses.on_tool_call_update(&update);

        let lines = statuses.render(&ctx());
        assert_eq!(lines.len(), 1);
        let text = lines[0].plain_text();
        assert!(
            text.contains("(Cargo.toml, 156 lines)"),
            "Completion meta should override preview: {text}"
        );
    }

    #[test]
    fn test_title_update_used_without_display_meta() {
        let mut statuses = ToolCallStatuses::new();
        statuses.on_tool_call(&make_tool_call(
            "tool-1",
            "coding__read_file",
            Some(r#"{"filePath":"Cargo.toml"}"#),
        ));
        let update = acp::ToolCallUpdate::new(
            "tool-1".to_string(),
            acp::ToolCallUpdateFields::new()
                .title("Read file")
                .status(acp::ToolCallStatus::InProgress),
        );
        statuses.on_tool_call_update(&update);

        let lines = statuses.render(&ctx());
        assert_eq!(lines.len(), 1);
        let text = lines[0].plain_text();
        assert!(text.contains("Read file"), "Expected updated title: {text}");
    }

    #[test]
    fn test_native_title_used_directly() {
        let mut statuses = ToolCallStatuses::new();
        statuses.on_tool_call(&make_tool_call(
            "tool-1",
            "coding__read_file",
            Some(r#"{"filePath":"Cargo.toml"}"#),
        ));

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
        assert!(text.contains("Read file"), "Expected native title: {text}");
    }

    #[test]
    fn test_no_display_value_falls_back_to_args() {
        let mut statuses = ToolCallStatuses::new();
        statuses.on_tool_call(&make_tool_call(
            "tool-1",
            "external_tool",
            Some(r#"{"key":"value"}"#),
        ));

        // Complete without display_value
        statuses.on_tool_call_update(&make_tool_call_update(
            "tool-1",
            acp::ToolCallStatus::Completed,
        ));

        let lines = statuses.render(&ctx());
        assert_eq!(lines.len(), 1);
        let text = lines[0].plain_text();
        // Should show raw args since no display_value
        assert!(text.contains("key"), "Expected raw args in output: {text}");
    }

    #[test]
    fn view_renders_diff_preview_on_success() {
        let status = ToolCallStatus::Success;
        let diff_preview = DiffPreview {
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
            lang_hint: String::new(),
            start_line: None,
        };
        let view = ToolCallStatusView {
            name: "Edit file",
            arguments: "{}",
            display_value: Some("main.rs"),
            diff_preview: Some(&diff_preview),
            status: &status,
            tick: 0,
        };
        let lines = view.render(&ctx());
        // 1 status line + 2 diff lines (1 removed + 1 added)
        assert_eq!(lines.len(), 3);
        assert!(lines[0].plain_text().contains("✓"));
        assert!(lines[1].plain_text().contains("- old line"));
        assert!(lines[2].plain_text().contains("+ new line"));
    }

    #[test]
    fn view_hides_diff_preview_while_running() {
        let status = ToolCallStatus::Running;
        let diff_preview = DiffPreview {
            lines: vec![
                DiffLine {
                    tag: DiffTag::Removed,
                    content: "old".to_string(),
                },
                DiffLine {
                    tag: DiffTag::Added,
                    content: "new".to_string(),
                },
            ],
            lang_hint: String::new(),
            start_line: None,
        };
        let view = ToolCallStatusView {
            name: "Edit file",
            arguments: "{}",
            display_value: Some("main.rs"),
            diff_preview: Some(&diff_preview),
            status: &status,
            tick: 0,
        };
        let lines = view.render(&ctx());
        assert_eq!(lines.len(), 1, "Diff should not render while running");
    }

    #[test]
    fn diff_preview_rendered_in_render_tool() {
        let mut statuses = ToolCallStatuses::new();
        statuses.on_tool_call(&make_tool_call("tool-1", "coding__edit_file", None));

        let mut meta_map = serde_json::Map::new();
        meta_map.insert("display_value".into(), "main.rs".into());
        let diff = acp::Diff::new("/tmp/main.rs", "new\n").old_text("old\n".to_string());
        let update = acp::ToolCallUpdate::new(
            "tool-1".to_string(),
            acp::ToolCallUpdateFields::new()
                .title("Edit file")
                .status(acp::ToolCallStatus::Completed)
                .content(vec![
                    acp::ToolCallContent::Content(acp::Content::new(acp::ContentBlock::Text(
                        acp::TextContent::new("ok"),
                    ))),
                    acp::ToolCallContent::Diff(diff),
                ]),
        )
        .meta(meta_map);
        statuses.on_tool_call_update(&update);

        let lines = statuses.render_tool("tool-1", &ctx());
        // 1 status line + 2 diff lines (1 removed + 1 added)
        assert_eq!(lines.len(), 3);
        assert!(lines[0].plain_text().contains("Edit file"));
        assert!(lines[1].plain_text().contains("- old"));
        assert!(lines[2].plain_text().contains("+ new"));
    }

    #[test]
    fn progress_empty() {
        let statuses = ToolCallStatuses::new();
        let progress = statuses.progress();
        assert_eq!(progress.completed_top_level, 0);
        assert_eq!(progress.total_top_level, 0);
        assert!(!progress.running_any);
    }

    #[test]
    fn progress_with_running_tools() {
        let mut statuses = ToolCallStatuses::new();
        statuses.on_tool_call(&make_tool_call("tool-1", "Read", None));
        statuses.on_tool_call(&make_tool_call("tool-2", "Write", None));
        let progress = statuses.progress();
        assert_eq!(progress.completed_top_level, 0);
        assert_eq!(progress.total_top_level, 2);
        assert!(progress.running_any);
    }

    #[test]
    fn progress_with_mixed_status() {
        let mut statuses = ToolCallStatuses::new();
        statuses.on_tool_call(&make_tool_call("tool-1", "Read", None));
        statuses.on_tool_call(&make_tool_call("tool-2", "Write", None));
        statuses.on_tool_call(&make_tool_call("tool-3", "Grep", None));
        statuses.on_tool_call_update(&make_tool_call_update(
            "tool-1",
            acp::ToolCallStatus::Completed,
        ));
        statuses.on_tool_call_update(&make_tool_call_update(
            "tool-3",
            acp::ToolCallStatus::Failed,
        ));
        let progress = statuses.progress();
        assert_eq!(progress.completed_top_level, 2);
        assert_eq!(progress.total_top_level, 3);
        assert!(progress.running_any);
    }

    #[test]
    fn parent_tool_hidden_when_sub_agents_exist() {
        let mut statuses = ToolCallStatuses::new();
        statuses.on_tool_call(&make_tool_call("parent-1", "spawn_subagent", None));

        statuses.on_sub_agent_progress(&make_sub_agent_notification(
            "parent-1",
            "explorer",
            r#"{"ToolCall":{"request":{"id":"c1","name":"grep","arguments":"{}"},"model_name":"m"}}"#,
        ));

        let lines = statuses.render_tool("parent-1", &ctx());
        let all_text: String = lines.iter().map(Line::plain_text).collect();
        assert!(
            !all_text.contains("spawn_subagent"),
            "Parent tool line should be hidden when sub-agents exist: {all_text}"
        );
    }

    #[test]
    fn parent_tool_shown_before_sub_agent_events() {
        let mut statuses = ToolCallStatuses::new();
        statuses.on_tool_call(&make_tool_call("parent-1", "spawn_subagent", None));

        // No sub-agent events yet — parent spinner should render normally
        let lines = statuses.render_tool("parent-1", &ctx());
        assert_eq!(lines.len(), 1);
        assert!(lines[0].plain_text().contains("spawn_subagent"));
    }

    #[test]
    fn progress_excludes_sub_agent_parents() {
        let mut statuses = ToolCallStatuses::new();
        statuses.on_tool_call(&make_tool_call("parent-1", "spawn_subagent", None));

        statuses.on_sub_agent_progress(&make_sub_agent_notification(
            "parent-1",
            "explorer",
            r#"{"ToolCall":{"request":{"id":"c1","name":"grep","arguments":"{}"},"model_name":"m"}}"#,
        ));

        let progress = statuses.progress();
        assert_eq!(
            progress.total_top_level, 0,
            "Sub-agent parent should be excluded from top-level count"
        );
    }

    #[test]
    fn completed_tool_is_not_active_for_render() {
        let mut statuses = ToolCallStatuses::new();
        statuses.on_tool_call(&make_tool_call("tool-1", "Read", None));
        statuses.on_tool_call_update(&make_tool_call_update(
            "tool-1",
            acp::ToolCallStatus::Completed,
        ));

        assert!(!statuses.is_tool_active_for_render("tool-1"));
    }

    #[test]
    fn completed_parent_with_running_sub_agent_stays_active_for_render() {
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

        assert!(statuses.is_tool_active_for_render("parent-1"));
    }

    #[test]
    fn completed_parent_with_done_sub_agent_is_not_active_for_render() {
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
        statuses.on_sub_agent_progress(&make_sub_agent_notification(
            "parent-1",
            "explorer",
            r#"{"ToolResult":{"result":{"id":"c1","name":"grep","arguments":"{}","result":"ok"},"model_name":"m"}}"#,
        ));
        statuses.on_sub_agent_progress(&make_sub_agent_notification(
            "parent-1",
            "explorer",
            r#""Done""#,
        ));

        assert!(!statuses.is_tool_active_for_render("parent-1"));
    }

    #[test]
    fn progress_stays_running_until_sub_agent_done() {
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
        statuses.on_sub_agent_progress(&make_sub_agent_notification(
            "parent-1",
            "explorer",
            r#"{"ToolResult":{"result":{"id":"c1","name":"grep","arguments":"{}","result":"ok"},"model_name":"m"}}"#,
        ));

        assert!(
            statuses.progress().running_any,
            "undone sub-agent should keep ticks alive for spinner animation"
        );

        statuses.on_sub_agent_progress(&make_sub_agent_notification(
            "parent-1",
            "explorer",
            r#""Done""#,
        ));

        assert!(!statuses.progress().running_any);
    }

    #[test]
    fn sub_agent_tool_call_update_without_preceding_call_does_not_double_arguments() {
        let mut statuses = ToolCallStatuses::new();
        statuses.on_tool_call(&make_tool_call("parent-1", "spawn_subagent", None));

        // ToolCallUpdate arrives WITHOUT a preceding ToolCall for "c1"
        statuses.on_sub_agent_progress(&make_sub_agent_notification(
            "parent-1",
            "explorer",
            r#"{"ToolCallUpdate":{"update":{"id":"c1","chunk":"ABC"}}}"#,
        ));

        // Complete so arguments render
        statuses.on_sub_agent_progress(&make_sub_agent_notification(
            "parent-1",
            "explorer",
            r#"{"ToolResult":{"result":{"id":"c1","name":"tool","arguments":"{}","result":"ok"},"model_name":"m"}}"#,
        ));

        let lines = statuses.render_tool("parent-1", &ctx());
        let text = lines[1].plain_text();
        assert!(text.contains("ABC"), "should contain chunk");
        assert!(!text.contains("ABCABC"), "arguments doubled: {text}");
    }
}
