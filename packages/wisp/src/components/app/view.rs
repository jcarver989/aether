use super::git_diff_mode::GitDiffMode;
use super::state::UiState;
use super::ScreenMode;
use crate::components::conversation_window::ConversationWindow;
use crate::components::plan_view::PlanView;
use crate::components::status_line::StatusLine;
use crate::tui::advanced::Renderer;
use crate::tui::{Component, Cursor, Frame, Layout, Line, ViewContext};
use acp_utils::notifications::McpServerStatus;
use agent_client_protocol as acp;
use std::io::{self, Write};
use std::time::Instant;
use utils::ReasoningEffort;

pub struct UiView<W: Write> {
    renderer: Renderer<W>,
    pub(crate) git_diff_mode: GitDiffMode,
    cached_visible_plan_entries: Vec<acp::PlanEntry>,
    cached_plan_version: u64,
    cached_plan_tick: Instant,
}

impl<W: Write> UiView<W> {
    pub fn new(renderer: Renderer<W>, git_diff_mode: GitDiffMode) -> Self {
        Self {
            renderer,
            git_diff_mode,
            cached_visible_plan_entries: Vec::new(),
            cached_plan_version: 0,
            cached_plan_tick: Instant::now(),
        }
    }

    pub fn render(&mut self, state: &mut UiState) -> Result<(), io::Error> {
        self.prepare_for_view(state);
        let state: &UiState = state;
        let git_diff_mode = &self.git_diff_mode;
        let plan_entries = &self.cached_visible_plan_entries;
        self.renderer
            .render_frame(|ctx| build_frame(state, git_diff_mode, plan_entries, ctx))
    }

    pub(crate) fn push_scrollback(&mut self, lines: &[Line]) -> Result<(), io::Error> {
        self.renderer.push_to_scrollback(lines)
    }

    pub(crate) fn clear_screen(&mut self) -> Result<(), io::Error> {
        self.renderer.clear_screen()
    }

    pub fn on_resize(&mut self, size: (u16, u16)) {
        self.renderer.on_resize(size);
    }

    pub(crate) fn context(&self) -> ViewContext {
        self.renderer.context()
    }

    pub(crate) fn apply_theme_selection(&mut self, file: Option<String>) {
        let mut settings = crate::settings::load_or_create_settings();
        settings.theme.file = file;

        if let Err(err) = crate::settings::save_settings(&settings) {
            tracing::warn!("Failed to persist theme setting: {err}");
        }

        let theme = crate::settings::load_theme(&settings);
        self.renderer.set_theme(theme);
    }

    pub fn renderer(&self) -> &Renderer<W> {
        &self.renderer
    }

    pub fn renderer_mut(&mut self) -> &mut Renderer<W> {
        &mut self.renderer
    }

    fn prepare_for_view(&mut self, state: &mut UiState) {
        let context = self.context();
        state.refresh_caches(&context, Some(&mut self.git_diff_mode));

        if let Some(ref mut overlay) = state.config_overlay {
            let height = (context.size.height.saturating_sub(1)) as usize;
            if height >= 3 {
                overlay.update_child_viewport(height.saturating_sub(4));
            }
        }

        let plan_version = state.plan_tracker.version();
        let last_tick = state.plan_tracker.last_tick();
        if plan_version != self.cached_plan_version || last_tick != self.cached_plan_tick {
            let grace_period = state.plan_tracker.grace_period;
            self.cached_visible_plan_entries =
                state.plan_tracker.visible_entries(last_tick, grace_period);
            self.cached_plan_version = plan_version;
            self.cached_plan_tick = last_tick;
        }
    }
}

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

pub(super) fn build_frame(
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::{ThemeSettings as WispThemeSettings, WispSettings, save_settings};
    use crate::test_helpers::{CUSTOM_TMTHEME, with_wisp_home};
    use crate::tui::{Color, Theme};
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn apply_theme_selection_persists_and_applies_theme_file() {
        let temp_dir = TempDir::new().unwrap();
        let themes_dir = temp_dir.path().join("themes");
        fs::create_dir_all(&themes_dir).unwrap();
        fs::write(themes_dir.join("custom.tmTheme"), CUSTOM_TMTHEME).unwrap();

        with_wisp_home(temp_dir.path(), || {
            let renderer = Renderer::new(Vec::new(), Theme::default());
            let git_diff_mode = GitDiffMode::new(std::path::PathBuf::from("."));
            let mut view = UiView::new(renderer, git_diff_mode);
            view.apply_theme_selection(Some("custom.tmTheme".to_string()));

            assert_eq!(
                view.context().theme.text_primary(),
                Color::Rgb {
                    r: 0x11,
                    g: 0x22,
                    b: 0x33
                }
            );

            let loaded = crate::settings::load_or_create_settings();
            assert_eq!(loaded.theme.file.as_deref(), Some("custom.tmTheme"));
        });
    }

    #[test]
    fn apply_theme_selection_persists_default_theme_as_none() {
        let temp_dir = TempDir::new().unwrap();
        with_wisp_home(temp_dir.path(), || {
            save_settings(&WispSettings {
                theme: WispThemeSettings {
                    file: Some("old.tmTheme".to_string()),
                },
            })
            .unwrap();

            let renderer = Renderer::new(Vec::new(), Theme::default());
            let git_diff_mode = GitDiffMode::new(std::path::PathBuf::from("."));
            let mut view = UiView::new(renderer, git_diff_mode);
            view.apply_theme_selection(None);

            let loaded = crate::settings::load_or_create_settings();
            assert_eq!(loaded.theme.file, None);
        });
    }
}
