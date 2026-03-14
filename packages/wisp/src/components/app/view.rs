use super::App;
use crate::components::status_line::StatusLine;
use crate::tui::{Component, Frame, Layout, ViewContext};

pub fn build_frame(app: &App, context: &ViewContext) -> Frame {
    let status_line = StatusLine {
        agent_name: &app.agent_name,
        config_options: app.config_manager.config_options(),
        context_pct_left: app.context_usage_pct,
        waiting_for_response: app.conversation_screen.is_waiting(),
        unhealthy_server_count: app.config_manager.unhealthy_server_count(),
    };

    if let Some(overlay_frame) = app.config_manager.build_overlay_frame(context) {
        let mut layout = Layout::new();
        layout.section_with_cursor(overlay_frame.lines().to_vec(), overlay_frame.cursor());
        layout.section(status_line.render(context));
        return layout.into_frame();
    }

    if app.screen_router.is_git_diff() {
        let status_lines = status_line.render(context);
        let diff_frame = app.screen_router.render(context);

        let mut layout = Layout::new();
        layout.section_with_cursor(diff_frame.lines().to_vec(), diff_frame.cursor());
        layout.section(status_lines);
        return layout.into_frame();
    }

    let conv_frame = app.conversation_screen.render(context);
    let mut layout = Layout::new();
    layout.section_with_cursor(conv_frame.lines().to_vec(), conv_frame.cursor());
    layout.section(status_line.render(context));
    layout.into_frame()
}
