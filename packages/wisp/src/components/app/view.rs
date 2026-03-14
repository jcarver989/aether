use super::git_diff_mode::GitDiffMode;
use super::state::UiState;
use super::ScreenMode;
use crate::components::conversation_window::ConversationWindow;
use crate::components::plan_view::PlanView;
use crate::components::status_line::StatusLine;
use crate::tui::{Component, Cursor, Frame, Layout, ViewContext};
use acp_utils::notifications::McpServerStatus;
use agent_client_protocol as acp;
use utils::ReasoningEffort;

struct StatusLineProps {
    agent_name: String,
    mode_display: Option<String>,
    model_display: Option<String>,
    reasoning_effort: Option<ReasoningEffort>,
    context_pct_left: Option<u8>,
    waiting_for_response: bool,
    unhealthy_server_count: usize,
}

fn status_line_props(state: &UiState) -> StatusLineProps {
    let unhealthy_count = state
        .server_statuses
        .iter()
        .filter(|status| !matches!(status.status, McpServerStatus::Connected { .. }))
        .count();
    StatusLineProps {
        agent_name: state.agent_name.clone(),
        mode_display: state.mode_display.clone(),
        model_display: state.model_display.clone(),
        reasoning_effort: state.reasoning_effort,
        context_pct_left: state.context_usage_pct,
        waiting_for_response: state.waiting_for_response,
        unhealthy_server_count: unhealthy_count,
    }
}

pub fn build_frame(
    state: &UiState,
    git_diff_mode: &GitDiffMode,
    plan_entries: &[acp::PlanEntry],
    context: &ViewContext,
) -> Frame {
    let s = status_line_props(state);
    let status_line = StatusLine {
        agent_name: &s.agent_name,
        mode_display: s.mode_display.as_deref(),
        model_display: s.model_display.as_deref(),
        reasoning_effort: s.reasoning_effort,
        context_pct_left: s.context_pct_left,
        waiting_for_response: s.waiting_for_response,
        unhealthy_server_count: s.unhealthy_server_count,
    };

    if let Some(ref overlay) = state.config_overlay {
        let cursor = if overlay.has_picker() {
            Cursor::visible(overlay.cursor_row_offset(), overlay.cursor_col())
        } else {
            Cursor::hidden()
        };

        let mut layout = Layout::new();
        layout.section(overlay.render(context));
        layout.section(status_line.render(context));
        return layout.into_frame().with_cursor(cursor);
    }

    if matches!(state.screen_mode, ScreenMode::GitDiff) {
        let status_lines = status_line.render(context);
        #[allow(clippy::cast_possible_truncation)]
        let diff_height = context
            .size
            .height
            .saturating_sub(status_lines.len() as u16);
        let diff_context = context.with_size((context.size.width, diff_height));
        let line_count = diff_height as usize;

        let cursor = if git_diff_mode.is_comment_input() {
            let comment_cursor = git_diff_mode.comment_cursor_col();
            Cursor::visible(
                line_count.saturating_sub(1),
                "Comment: ".len() + comment_cursor,
            )
        } else {
            Cursor::hidden()
        };

        let mut layout = Layout::new();
        layout.section(git_diff_mode.render_lines(&diff_context));
        layout.section(status_lines);
        return layout.into_frame().with_cursor(cursor);
    }

    let conversation_window = ConversationWindow {
        loader: &state.grid_loader,
        conversation: &state.conversation,
    };
    let plan_view = PlanView {
        entries: plan_entries,
    };

    let mut layout = Layout::new();
    layout.section(conversation_window.render(context));
    layout.section(plan_view.render(context));
    layout.section(state.progress_indicator.render(context));
    layout.section_with_cursor(
        state.prompt_composer.render(context),
        state.prompt_composer.cursor(context),
    );
    if let Some(ref session_picker) = state.session_picker {
        layout.section(session_picker.render(context));
    }
    if let Some(ref elicitation_form) = state.elicitation_form {
        layout.section(elicitation_form.form.render(context));
    }
    layout.section(status_line.render(context));
    layout.into_frame()
}
