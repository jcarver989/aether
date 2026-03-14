use super::git_diff_mode::{GitDiffMode, ScreenMode};
use crate::components::git_diff_view::GitDiffViewMessage;
use crate::tui::{Component, Cursor, Event, Frame, Line, ViewContext};

const STATUS_LINE_HEIGHT: u16 = 1;

pub enum ScreenRouterMessage {
    LoadGitDiff,
    RefreshGitDiff,
    SendPrompt { user_input: String },
}

pub struct ScreenRouter {
    screen_mode: ScreenMode,
    git_diff_mode: GitDiffMode,
}

impl ScreenRouter {
    pub fn new(git_diff_mode: GitDiffMode) -> Self {
        Self {
            screen_mode: ScreenMode::Conversation,
            git_diff_mode,
        }
    }

    pub fn is_git_diff(&self) -> bool {
        matches!(self.screen_mode, ScreenMode::GitDiff)
    }

    pub fn toggle_git_diff(&mut self) -> Option<ScreenRouterMessage> {
        if self.is_git_diff() {
            self.git_diff_mode.close();
            self.screen_mode = ScreenMode::Conversation;
            None
        } else {
            self.screen_mode = ScreenMode::GitDiff;
            self.git_diff_mode.begin_open();
            Some(ScreenRouterMessage::LoadGitDiff)
        }
    }

    pub fn refresh_caches(&mut self, context: &ViewContext) {
        if self.is_git_diff() {
            self.git_diff_mode.refresh_caches(context);
        }
    }

    pub fn git_diff_mode_mut(&mut self) -> &mut GitDiffMode {
        &mut self.git_diff_mode
    }

    #[cfg(test)]
    pub fn enter_git_diff_for_test(&mut self) {
        self.screen_mode = ScreenMode::GitDiff;
    }
}

impl Component for ScreenRouter {
    type Message = ScreenRouterMessage;

    fn on_event(&mut self, event: &Event) -> Option<Vec<ScreenRouterMessage>> {
        let git_messages = self.git_diff_mode.on_key_event(event);
        let mut router_messages = Vec::new();
        for msg in git_messages {
            match msg {
                GitDiffViewMessage::Close => {
                    self.git_diff_mode.close();
                    self.screen_mode = ScreenMode::Conversation;
                }
                GitDiffViewMessage::Refresh => {
                    self.git_diff_mode.begin_refresh();
                    router_messages.push(ScreenRouterMessage::RefreshGitDiff);
                }
                GitDiffViewMessage::SubmitPrompt(user_input) => {
                    self.git_diff_mode.close();
                    self.screen_mode = ScreenMode::Conversation;
                    router_messages.push(ScreenRouterMessage::SendPrompt { user_input });
                }
            }
        }
        Some(router_messages)
    }

    fn render(&self, ctx: &ViewContext) -> Vec<Line> {
        let diff_height = ctx.size.height.saturating_sub(STATUS_LINE_HEIGHT);
        let diff_context = ctx.with_size((ctx.size.width, diff_height));
        self.git_diff_mode.render_lines(&diff_context)
    }

    fn cursor(&self, ctx: &ViewContext) -> Cursor {
        let diff_height = ctx.size.height.saturating_sub(STATUS_LINE_HEIGHT);
        let line_count = diff_height as usize;

        if self.git_diff_mode.is_comment_input() {
            let comment_cursor = self.git_diff_mode.comment_cursor_col();
            Cursor::visible(
                line_count.saturating_sub(1),
                "Comment: ".len() + comment_cursor,
            )
        } else {
            Cursor::hidden()
        }
    }

    fn build_frame(&self, ctx: &ViewContext) -> Frame {
        Frame::new(self.render(ctx), self.cursor(ctx))
    }
}
