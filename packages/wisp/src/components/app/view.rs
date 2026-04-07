use super::App;
use crate::components::status_line::StatusLine;
use crate::settings;
use tui::{Component, Frame, Layout, ViewContext};

pub fn build_frame(app: &mut App, context: &ViewContext) -> Frame {
    if let Some(ref mut overlay) = app.settings_overlay {
        let overlay_frame = overlay.build_frame(context);

        let status_line = make_status_line(app);
        let mut layout = Layout::new();
        layout.section(overlay_frame);
        layout.section(Frame::new(status_line.render(context)));
        return layout.into_frame();
    }

    if app.screen_router.is_git_diff() {
        let diff_frame = app.screen_router.render(context);
        let status_line = make_status_line(app);
        let status_lines = status_line.render(context);

        let mut layout = Layout::new();
        layout.section(diff_frame);
        layout.section(Frame::new(status_lines));
        return layout.into_frame();
    }

    let conv_frame = app.conversation_screen.render(context);
    let status_line = make_status_line(app);
    let mut layout = Layout::new();
    layout.section(conv_frame);
    layout.section(Frame::new(status_line.render(context)));
    layout.into_frame()
}

fn make_status_line(app: &App) -> StatusLine<'_> {
    StatusLine {
        agent_name: &app.agent_name,
        config_options: &app.config_options,
        context_pct_left: app.context_usage_pct,
        waiting_for_response: app.conversation_screen.is_waiting(),
        unhealthy_server_count: settings::unhealthy_server_count(&app.server_statuses),
        content_padding: app.content_padding,
    }
}
