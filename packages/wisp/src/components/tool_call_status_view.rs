use agent_client_protocol as acp;
use similar::{ChangeTag, TextDiff};
use std::path::Path;

use crate::tui::{DiffLine, DiffPreview, DiffTag, Line, ViewContext, highlight_diff};
use crate::tui::BRAILLE_FRAMES as FRAMES;

pub(crate) const MAX_TOOL_ARG_LENGTH: usize = 200;

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

        let display_text = self
            .display_value
            .filter(|v| !v.is_empty())
            .map_or_else(
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
