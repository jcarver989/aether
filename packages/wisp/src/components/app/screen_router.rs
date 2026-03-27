use super::git_diff_mode::{GitDiffMode, ScreenMode};
use crate::components::git_diff_view::GitDiffViewMessage;
use tui::{Component, Cursor, Event, Frame, ViewContext};

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
            self.close_git_diff();
            None
        } else {
            self.screen_mode = ScreenMode::GitDiff;
            self.git_diff_mode.begin_open();
            Some(ScreenRouterMessage::LoadGitDiff)
        }
    }

    pub fn close_git_diff(&mut self) {
        if self.is_git_diff() {
            self.git_diff_mode.close();
            self.screen_mode = ScreenMode::Conversation;
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

    async fn on_event(&mut self, event: &Event) -> Option<Vec<ScreenRouterMessage>> {
        let git_messages = self.git_diff_mode.on_key_event(event).await;
        let mut router_messages = Vec::new();
        for msg in git_messages {
            match msg {
                GitDiffViewMessage::Close => {
                    self.close_git_diff();
                }
                GitDiffViewMessage::Refresh => {
                    self.git_diff_mode.begin_refresh();
                    router_messages.push(ScreenRouterMessage::RefreshGitDiff);
                }
                GitDiffViewMessage::SubmitPrompt(user_input) => {
                    router_messages.push(ScreenRouterMessage::SendPrompt { user_input });
                }
            }
        }
        Some(router_messages)
    }

    fn render(&mut self, ctx: &ViewContext) -> Frame {
        let diff_height = ctx.size.height.saturating_sub(STATUS_LINE_HEIGHT);
        let diff_context = ctx.with_size((ctx.size.width, diff_height));
        let lines = self.git_diff_mode.render_lines(&diff_context);

        let cursor = if self.git_diff_mode.is_comment_input() {
            let line_count = diff_height as usize;
            let comment_cursor = self.git_diff_mode.comment_cursor_col();
            Cursor::visible(
                line_count.saturating_sub(1),
                "Comment: ".len() + comment_cursor,
            )
        } else {
            Cursor::hidden()
        };

        Frame::new(lines).with_cursor(cursor)
    }
}
