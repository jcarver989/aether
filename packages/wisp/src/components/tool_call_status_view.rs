use agent_client_protocol as acp;
use similar::{ChangeTag, TextDiff};
use std::collections::HashMap;
use std::path::Path;

use crate::components::sub_agent_tracker::{
    SUB_AGENT_VISIBLE_TOOL_LIMIT, SubAgentState, SubAgentTracker,
};
use crate::components::tracked_tool_call::TrackedToolCall;
use crate::tui::BRAILLE_FRAMES as FRAMES;
use crate::tui::{DiffLine, DiffPreview, DiffTag, Line, ViewContext, highlight_diff};

pub const MAX_TOOL_ARG_LENGTH: usize = 200;

/// Render a tool call and its sub-agent hierarchy (if any) as status lines.
pub(crate) fn render_tool_tree(
    id: &str,
    tool_calls: &HashMap<String, TrackedToolCall>,
    sub_agents: &SubAgentTracker,
    tick: u16,
    context: &ViewContext,
) -> Vec<Line> {
    let has_sub_agents = sub_agents.has_sub_agents(id);

    let mut lines = if has_sub_agents {
        Vec::new()
    } else {
        tool_calls
            .get(id)
            .map(|tc| tool_call_view(tc, tick).render(context))
            .unwrap_or_default()
    };

    if let Some(agents) = sub_agents.get(id) {
        for (i, agent) in agents.iter().enumerate() {
            if i > 0 {
                lines.push(Line::default());
            }
            lines.push(render_agent_header(agent, tick, context));

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

                let view = tool_call_view(tc, tick);
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

pub(crate) fn tool_call_view(tc: &TrackedToolCall, tick: u16) -> ToolCallStatusView<'_> {
    ToolCallStatusView {
        name: &tc.name,
        arguments: &tc.arguments,
        display_value: tc.display_value.as_deref(),
        diff_preview: tc.diff_preview.as_ref(),
        status: &tc.status,
        tick,
    }
}

/// Renders a single tool call status line.
pub struct ToolCallStatusView<'a> {
    pub name: &'a str,
    pub arguments: &'a str,
    pub display_value: Option<&'a str>,
    pub diff_preview: Option<&'a DiffPreview>,
    pub status: &'a ToolCallStatus,
    pub tick: u16,
}

#[derive(Clone)]
pub enum ToolCallStatus {
    Running,
    Success,
    Error(String),
}

impl ToolCallStatusView<'_> {
    pub fn render(&self, context: &ViewContext) -> Vec<Line> {
        let (indicator, indicator_color) = match &self.status {
            ToolCallStatus::Running => {
                let frame = FRAMES[self.tick as usize % FRAMES.len()];
                (frame.to_string(), context.theme.info())
            }
            ToolCallStatus::Success => ("✓".to_string(), context.theme.success()),
            ToolCallStatus::Error(_) => ("✗".to_string(), context.theme.error()),
        };

        let mut line = Line::default();
        line.push_styled(indicator, indicator_color);
        line.push_text(" ");
        line.push_text(self.name);

        let display_text = self.display_value.filter(|v| !v.is_empty()).map_or_else(
            || match self.status {
                ToolCallStatus::Running => String::new(),
                _ => format_arguments(self.arguments),
            },
            |v| format!(" ({v})"),
        );
        line.push_styled(display_text, context.theme.muted());

        if let ToolCallStatus::Error(msg) = &self.status {
            line.push_text(" ");
            line.push_styled(msg, context.theme.error());
        }

        let mut lines = vec![line];

        if matches!(self.status, ToolCallStatus::Success)
            && let Some(preview) = self.diff_preview
        {
            lines.extend(highlight_diff(preview, context));
        }

        lines
    }
}

/// Compute a visual diff preview from an ACP `Diff` (full old/new text).
pub(super) fn compute_diff_preview(diff: &acp::Diff) -> DiffPreview {
    let old_text = diff.old_text.as_deref().unwrap_or("");
    let text_diff = TextDiff::from_lines(old_text, &diff.new_text);

    let mut lines = Vec::new();
    let mut first_change_line: Option<usize> = None;
    let mut current_old_line: usize = 0;

    for change in text_diff.iter_all_changes() {
        let tag = match change.tag() {
            ChangeTag::Equal => {
                current_old_line += 1;
                DiffTag::Context
            }
            ChangeTag::Delete => {
                if first_change_line.is_none() {
                    first_change_line = Some(current_old_line + 1);
                }
                current_old_line += 1;
                DiffTag::Removed
            }
            ChangeTag::Insert => {
                if first_change_line.is_none() {
                    first_change_line = Some(current_old_line + 1);
                }
                DiffTag::Added
            }
        };
        lines.push(DiffLine {
            tag,
            content: change.value().trim_end_matches('\n').to_string(),
        });
    }

    const CONTEXT_LINES: usize = 3;

    let first_change_idx = lines.iter().position(|l| l.tag != DiffTag::Context);
    let last_change_idx = lines.iter().rposition(|l| l.tag != DiffTag::Context);

    if let (Some(first), Some(last)) = (first_change_idx, last_change_idx) {
        let start = first.saturating_sub(CONTEXT_LINES);
        let end = (last + CONTEXT_LINES + 1).min(lines.len());
        lines = lines[start..end].to_vec();
        let trimmed_context = first - start;
        first_change_line = first_change_line.map(|l| l - trimmed_context);
    }

    let lang_hint = Path::new(&diff.path)
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("")
        .to_lowercase();

    DiffPreview {
        lines,
        lang_hint,
        start_line: first_change_line,
    }
}

fn render_agent_header(agent: &SubAgentState, tick: u16, context: &ViewContext) -> Line {
    let mut line = Line::default();
    line.push_text("  ");
    if agent.done {
        line.push_styled("✓".to_string(), context.theme.success());
    } else {
        let frame = FRAMES[tick as usize % FRAMES.len()];
        line.push_styled(frame.to_string(), context.theme.info());
    }
    line.push_text(" ");
    line.push_text(&agent.agent_name);
    line
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_large_file(num_lines: usize) -> String {
        (1..=num_lines)
            .map(|i| format!("line {i}"))
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn replace_line(text: &str, line_num: usize, replacement: &str) -> String {
        text.lines()
            .enumerate()
            .map(|(i, l)| if i + 1 == line_num { replacement } else { l })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn diff_preview_for_edit_near_end_contains_change() {
        let old = make_large_file(50);
        let new = replace_line(&old, 45, "CHANGED LINE 45");

        let diff = acp::Diff::new("test.rs", new).old_text(old);
        let preview = compute_diff_preview(&diff);

        let has_change = preview.lines.iter().any(|l| l.tag != DiffTag::Context);
        assert!(has_change, "preview must contain the changed lines");
    }

    #[test]
    fn diff_preview_trims_leading_context() {
        let old = make_large_file(50);
        let new = replace_line(&old, 45, "CHANGED LINE 45");

        let diff = acp::Diff::new("test.rs", new).old_text(old);
        let preview = compute_diff_preview(&diff);

        assert!(
            preview.lines.len() <= 10,
            "expected at most ~10 lines (3 context + change + 3 context), got {}",
            preview.lines.len()
        );
    }

    #[test]
    fn diff_preview_start_line_adjusted_after_trim() {
        let old = make_large_file(50);
        let new = replace_line(&old, 45, "CHANGED LINE 45");

        let diff = acp::Diff::new("test.rs", new).old_text(old);
        let preview = compute_diff_preview(&diff);

        let start = preview.start_line.expect("start_line should be set");
        assert!(
            start >= 42,
            "start_line should be near the edit (line 45), got {start}"
        );
    }
}

fn format_arguments(arguments: &str) -> String {
    let mut formatted = format!(" {arguments}");
    if formatted.len() > MAX_TOOL_ARG_LENGTH {
        let mut new_len = MAX_TOOL_ARG_LENGTH;
        while !formatted.is_char_boundary(new_len) {
            new_len -= 1;
        }
        formatted.truncate(new_len);
    }
    formatted
}
