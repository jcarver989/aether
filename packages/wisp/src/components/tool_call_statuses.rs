use acp_utils::notifications::{
    DiffPreview, SubAgentEvent, SubAgentProgressParams, ToolResultMeta,
};
use agent_client_protocol as acp;

use crate::tui::diff::highlight_diff;
use crate::tui::spinner::BRAILLE_FRAMES as FRAMES;
use crate::tui::{Component, Line, RenderContext};
use std::collections::HashMap;

const MAX_TOOL_ARG_LENGTH: usize = 200;
const SUB_AGENT_VISIBLE_TOOL_LIMIT: usize = 3;

/// Renders a single tool call status line.
pub struct ToolCallStatusView {
    pub name: String,
    pub arguments: String,
    pub display_value: Option<String>,
    pub diff_preview: Option<DiffPreview>,
    pub status: ToolCallStatus,
    pub tick: u16,
}

#[derive(Clone)]
pub enum ToolCallStatus {
    Running,
    Success,
    Error(String),
}

impl Component for ToolCallStatusView {
    fn render(&mut self, context: &RenderContext) -> Vec<Line> {
        let (indicator, indicator_color) = match &self.status {
            ToolCallStatus::Running => {
                let frame = FRAMES[self.tick as usize % FRAMES.len()];
                (frame.to_string(), context.theme.info)
            }
            ToolCallStatus::Success => ("✓".to_string(), context.theme.success),
            ToolCallStatus::Error(_) => ("✗".to_string(), context.theme.error),
        };

        let mut line = Line::default();
        line.push_styled(indicator, indicator_color);
        line.push_text(" ");
        line.push_text(&self.name);

        let display_text = self
            .display_value
            .as_ref()
            .filter(|v| !v.is_empty() && !matches!(self.status, ToolCallStatus::Running))
            .map_or_else(
                || Self::format_arguments(&self.arguments),
                |v| format!(" ({v})"),
            );
        line.push_styled(display_text, context.theme.muted);

        if let ToolCallStatus::Error(msg) = &self.status {
            line.push_text(" ");
            line.push_styled(msg, context.theme.error);
        }

        let mut lines = vec![line];

        if matches!(self.status, ToolCallStatus::Success)
            && let Some(ref preview) = self.diff_preview
        {
            lines.extend(highlight_diff(preview, &context.theme));
        }

        lines
    }
}

impl ToolCallStatusView {
    fn format_arguments(arguments: &str) -> String {
        let mut formatted = format!(" {arguments}");
        formatted.truncate(MAX_TOOL_ARG_LENGTH);
        formatted
    }
}

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
    result_meta: Option<ToolResultMeta>,
    status: ToolCallStatus,
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

    /// Update the animation tick for running tool call spinners.
    pub fn set_tick(&mut self, tick: u16) {
        self.tick = tick;
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
            || self.sub_agents.values().any(|agents| {
                agents.iter().any(|a| {
                    a.tool_calls
                        .values()
                        .any(|tc| matches!(tc.status, ToolCallStatus::Running))
                })
            })
    }

    /// Handle a new tool call from ACP `SessionUpdate::ToolCall`.
    pub fn on_tool_call(&mut self, tool_call: &acp::ToolCall) {
        let id = tool_call.tool_call_id.0.to_string();
        let arguments = tool_call
            .raw_input
            .as_ref()
            .map(std::string::ToString::to_string)
            .unwrap_or_default();

        if let Some(existing) = self.tool_calls.get_mut(&id) {
            if !tool_call.title.is_empty() {
                existing.name.clone_from(&tool_call.title);
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
                result_meta: None,
                status: ToolCallStatus::Running,
            },
        );
    }

    /// Handle a tool call update from ACP `SessionUpdate::ToolCallUpdate`.
    pub fn on_tool_call_update(&mut self, update: &acp::ToolCallUpdate) {
        let id = update.tool_call_id.0.to_string();

        if let Some(tc) = self.tool_calls.get_mut(&id) {
            if let Some(title) = &update.fields.title {
                tc.name.clone_from(title);
            }
            if let Some(raw_input) = &update.fields.raw_input {
                tc.arguments = raw_input.to_string();
            }
            if let Some(meta) = &update.meta
                && let Some(tc_meta) = ToolResultMeta::from_map(meta)
            {
                tc.name.clone_from(&tc_meta.display.title);
                tc.result_meta = Some(tc_meta);
            }
            if let Some(status) = &update.fields.status {
                match status {
                    acp::ToolCallStatus::Completed => tc.status = ToolCallStatus::Success,
                    acp::ToolCallStatus::Failed => {
                        tc.status = ToolCallStatus::Error("failed".to_string());
                    }
                    acp::ToolCallStatus::InProgress | acp::ToolCallStatus::Pending => {
                        tc.status = ToolCallStatus::Running;
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
            .is_some_and(|tc| matches!(tc.status, ToolCallStatus::Running))
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
                if !agent.tool_calls.contains_key(&request.id) {
                    agent.tool_order.push(request.id.clone());
                }
                agent.tool_calls.insert(
                    request.id.clone(),
                    TrackedToolCall {
                        name: request.name.clone(),
                        arguments: request.arguments.clone(),
                        result_meta: None,
                        status: ToolCallStatus::Running,
                    },
                );
            }
            SubAgentEvent::ToolResult { result } => {
                if let Some(tc) = agent.tool_calls.get_mut(&result.id) {
                    tc.status = ToolCallStatus::Success;
                    if let Some(result_meta) = &result.result_meta {
                        tc.name.clone_from(&result_meta.display.title);
                        tc.result_meta = Some(result_meta.clone());
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

    pub fn render_tool(&self, id: &str, context: &RenderContext) -> Vec<Line> {
        let has_sub_agents = self.has_sub_agents(id);

        let mut lines = if has_sub_agents {
            Vec::new()
        } else {
            self.view_for(id, self.tick)
                .map(|mut view| view.render(context))
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
                        context.theme.muted,
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

                    let mut view = Self::tool_call_view(tc, self.tick);
                    for tool_line in view.render(context) {
                        let mut indented = Line::default();
                        indented.push_styled(connector, context.theme.muted);
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
    #[allow(dead_code)]
    pub fn clear(&mut self) {
        self.tool_order.clear();
        self.tool_calls.clear();
        self.sub_agents.clear();
    }

    /// Render and remove only completed (Success/Error) tool calls,
    /// leaving Running ones in place for continued display.
    #[allow(dead_code)]
    pub fn drain_completed(&mut self, context: &RenderContext) -> Vec<Line> {
        let mut lines = Vec::new();
        let mut completed_ids = Vec::new();

        for id in &self.tool_order {
            if let Some(tc) = self.tool_calls.get(id)
                && !matches!(tc.status, ToolCallStatus::Running)
            {
                let mut view = Self::tool_call_view(tc, 0);
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

    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.tool_calls.is_empty()
    }

    fn render_agent_header(&self, agent: &SubAgentState, context: &RenderContext) -> Line {
        let mut line = Line::default();
        line.push_text("  ");
        if agent.done {
            line.push_styled("✓".to_string(), context.theme.success);
        } else {
            let frame = FRAMES[self.tick as usize % FRAMES.len()];
            line.push_styled(frame.to_string(), context.theme.info);
        }
        line.push_text(" ");
        line.push_text(&agent.agent_name);
        line
    }

    fn tool_call_view(tc: &TrackedToolCall, tick: u16) -> ToolCallStatusView {
        ToolCallStatusView {
            name: tc.name.clone(),
            arguments: tc.arguments.clone(),
            display_value: tc.result_meta.as_ref().map(|rm| rm.display.value.clone()),
            diff_preview: tc
                .result_meta
                .as_ref()
                .and_then(|rm| rm.diff_preview.clone()),
            status: tc.status.clone(),
            tick,
        }
    }

    fn view_for(&self, id: &str, tick: u16) -> Option<ToolCallStatusView> {
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

impl Component for ToolCallStatuses {
    fn render(&mut self, context: &RenderContext) -> Vec<Line> {
        let mut lines = Vec::new();
        for id in &self.tool_order {
            if let Some(mut view) = self.view_for(id, self.tick) {
                lines.extend(view.render(context));
            }
        }
        lines
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use acp_utils::notifications::ToolDisplayMeta;

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
        let mut view = ToolCallStatusView {
            name: "TestTool".to_string(),
            arguments: "test args".to_string(),
            display_value: None,
            diff_preview: None,
            status: ToolCallStatus::Running,
            tick: 0,
        };
        let lines = view.render(&ctx());
        assert_eq!(lines.len(), 1);
        let text = lines[0].plain_text();
        assert!(text.contains("TestTool"));
        assert!(text.contains("test args"));
        assert!(text.contains('⠋'));
    }

    #[test]
    fn view_running_spinner_changes_with_tick() {
        let mut view_a = ToolCallStatusView {
            name: "TestTool".to_string(),
            arguments: String::new(),
            display_value: None,
            diff_preview: None,
            status: ToolCallStatus::Running,
            tick: 0,
        };
        let mut view_b = ToolCallStatusView {
            name: "TestTool".to_string(),
            arguments: String::new(),
            display_value: None,
            diff_preview: None,
            status: ToolCallStatus::Running,
            tick: 1,
        };
        let a = view_a.render(&ctx())[0].plain_text();
        let b = view_b.render(&ctx())[0].plain_text();
        assert_ne!(a, b);
    }

    #[test]
    fn view_renders_success() {
        let mut view = ToolCallStatusView {
            name: "TestTool".to_string(),
            arguments: "test args".to_string(),
            display_value: None,
            diff_preview: None,
            status: ToolCallStatus::Success,
            tick: 0,
        };
        let lines = view.render(&ctx());
        assert_eq!(lines.len(), 1);
        assert!(lines[0].plain_text().contains("✓"));
    }

    #[test]
    fn view_renders_error() {
        let mut view = ToolCallStatusView {
            name: "TestTool".to_string(),
            arguments: "test args".to_string(),
            display_value: None,
            diff_preview: None,
            status: ToolCallStatus::Error("boom".to_string()),
            tick: 0,
        };
        let lines = view.render(&ctx());
        assert_eq!(lines.len(), 1);
        assert!(lines[0].plain_text().contains("✗"));
        assert!(lines[0].plain_text().contains("boom"));
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

        // Complete with display title/value in meta
        let meta = ToolResultMeta::from(ToolDisplayMeta::new("Read file", "Cargo.toml, 156 lines"))
            .into_map();
        let update = acp::ToolCallUpdate::new(
            "tool-1".to_string(),
            acp::ToolCallUpdateFields::new().status(acp::ToolCallStatus::Completed),
        )
        .meta(meta);
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
    fn test_display_value_not_shown_while_running() {
        let mut statuses = ToolCallStatuses::new();
        statuses.on_tool_call(&make_tool_call(
            "tool-1",
            "Read file",
            Some(r#"{"filePath":"Cargo.toml"}"#),
        ));

        // Send an update with display_value but keep running
        let meta = ToolResultMeta::from(ToolDisplayMeta::new("Read file", "Cargo.toml, 156 lines"))
            .into_map();
        let update = acp::ToolCallUpdate::new(
            "tool-1".to_string(),
            acp::ToolCallUpdateFields::new().status(acp::ToolCallStatus::InProgress),
        )
        .meta(meta);
        statuses.on_tool_call_update(&update);

        let lines = statuses.render(&ctx());
        assert_eq!(lines.len(), 1);
        let text = lines[0].plain_text();
        // While running, should show raw args, not display_value
        assert!(
            !text.contains("(Cargo.toml, 156 lines)"),
            "display_value should not appear while running: {text}"
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
    fn test_display_meta_title_overrides_update_title() {
        let mut statuses = ToolCallStatuses::new();
        statuses.on_tool_call(&make_tool_call(
            "tool-1",
            "coding__read_file",
            Some(r#"{"filePath":"Cargo.toml"}"#),
        ));

        let meta = ToolResultMeta::from(ToolDisplayMeta::new("Read file", "Cargo.toml, 156 lines"))
            .into_map();
        let update = acp::ToolCallUpdate::new(
            "tool-1".to_string(),
            acp::ToolCallUpdateFields::new()
                .title("Fallback title")
                .status(acp::ToolCallStatus::Completed),
        )
        .meta(meta);
        statuses.on_tool_call_update(&update);

        let lines = statuses.render(&ctx());
        assert_eq!(lines.len(), 1);
        let text = lines[0].plain_text();
        assert!(
            text.contains("Read file"),
            "Expected display_meta title: {text}"
        );
        assert!(
            !text.contains("Fallback title"),
            "Meta should win over title: {text}"
        );
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
        let mut view = ToolCallStatusView {
            name: "Edit file".to_string(),
            arguments: "{}".to_string(),
            display_value: Some("main.rs".to_string()),
            diff_preview: Some(DiffPreview {
                removed: vec!["old line".to_string()],
                added: vec!["new line".to_string()],
                lang_hint: String::new(),
                start_line: None,
            }),
            status: ToolCallStatus::Success,
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
        let mut view = ToolCallStatusView {
            name: "Edit file".to_string(),
            arguments: "{}".to_string(),
            display_value: Some("main.rs".to_string()),
            diff_preview: Some(DiffPreview {
                removed: vec!["old".to_string()],
                added: vec!["new".to_string()],
                lang_hint: String::new(),
                start_line: None,
            }),
            status: ToolCallStatus::Running,
            tick: 0,
        };
        let lines = view.render(&ctx());
        assert_eq!(lines.len(), 1, "Diff should not render while running");
    }

    #[test]
    fn diff_preview_rendered_in_render_tool() {
        let mut statuses = ToolCallStatuses::new();
        statuses.on_tool_call(&make_tool_call("tool-1", "coding__edit_file", None));

        // Complete with diff_preview in meta
        let rm = ToolResultMeta::with_diff_preview(
            ToolDisplayMeta::new("Edit file", "main.rs"),
            DiffPreview {
                removed: vec!["old".to_string()],
                added: vec!["new".to_string()],
                lang_hint: String::new(),
                start_line: None,
            },
        );
        let update = acp::ToolCallUpdate::new(
            "tool-1".to_string(),
            acp::ToolCallUpdateFields::new().status(acp::ToolCallStatus::Completed),
        )
        .meta(rm.into_map());
        statuses.on_tool_call_update(&update);

        let lines = statuses.render_tool("tool-1", &ctx());
        // 1 status line + 2 diff lines
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
        let all_text: String = lines.iter().map(|l| l.plain_text()).collect();
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
}
