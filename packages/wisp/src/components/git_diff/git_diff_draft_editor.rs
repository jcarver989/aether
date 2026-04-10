use super::git_diff_comment_renderer::DraftCommentState;
use crate::components::app::git_diff_mode::PatchLineRef;
use tui::KeyCode;

pub struct DraftCommentEditor {
    draft: Option<DraftCommentState>,
}

pub enum DraftCommentEdit {
    Noop,
    Cancelled,
    Submitted { anchor: PatchLineRef, text: String },
}

impl DraftCommentEditor {
    pub fn new() -> Self {
        Self { draft: None }
    }

    pub fn is_active(&self) -> bool {
        self.draft.is_some()
    }

    pub fn begin(&mut self, anchor: PatchLineRef) {
        self.draft = Some(DraftCommentState { anchor, text: String::new(), cursor_position: 0 });
    }

    pub fn cancel(&mut self) {
        self.draft = None;
    }

    pub fn state(&self) -> Option<DraftCommentState> {
        self.draft.clone()
    }

    pub fn handle_key(&mut self, code: KeyCode) -> DraftCommentEdit {
        let Some(draft) = self.draft.as_mut() else {
            return DraftCommentEdit::Noop;
        };

        match code {
            KeyCode::Esc => {
                self.cancel();
                DraftCommentEdit::Cancelled
            }
            KeyCode::Enter => {
                if draft.text.trim().is_empty() {
                    self.cancel();
                    DraftCommentEdit::Cancelled
                } else {
                    let submitted = DraftCommentEdit::Submitted { anchor: draft.anchor, text: draft.text.clone() };
                    self.cancel();
                    submitted
                }
            }
            KeyCode::Char(ch) => {
                let byte_pos = char_to_byte_pos(&draft.text, draft.cursor_position);
                draft.text.insert(byte_pos, ch);
                draft.cursor_position += 1;
                DraftCommentEdit::Noop
            }
            KeyCode::Backspace => {
                if draft.cursor_position > 0 {
                    draft.cursor_position -= 1;
                    let byte_pos = char_to_byte_pos(&draft.text, draft.cursor_position);
                    draft.text.remove(byte_pos);
                }
                DraftCommentEdit::Noop
            }
            KeyCode::Left => {
                draft.cursor_position = draft.cursor_position.saturating_sub(1);
                DraftCommentEdit::Noop
            }
            KeyCode::Right => {
                let max = draft.text.chars().count();
                draft.cursor_position = (draft.cursor_position + 1).min(max);
                DraftCommentEdit::Noop
            }
            _ => DraftCommentEdit::Noop,
        }
    }
}

impl Default for DraftCommentEditor {
    fn default() -> Self {
        Self::new()
    }
}

fn char_to_byte_pos(text: &str, char_idx: usize) -> usize {
    text.char_indices().nth(char_idx).map_or(text.len(), |(index, _)| index)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn submitting_empty_text_cancels_draft() {
        let anchor = PatchLineRef { hunk_index: 0, line_index: 1 };
        let mut editor = DraftCommentEditor::new();
        editor.begin(anchor);

        assert!(matches!(editor.handle_key(KeyCode::Enter), DraftCommentEdit::Cancelled));
        assert!(!editor.is_active());
    }

    #[test]
    fn inserting_and_submitting_returns_comment() {
        let anchor = PatchLineRef { hunk_index: 0, line_index: 1 };
        let mut editor = DraftCommentEditor::new();
        editor.begin(anchor);
        editor.handle_key(KeyCode::Char('h'));
        editor.handle_key(KeyCode::Char('i'));

        let submitted = editor.handle_key(KeyCode::Enter);
        assert!(matches!(submitted, DraftCommentEdit::Submitted { .. }));
    }
}
