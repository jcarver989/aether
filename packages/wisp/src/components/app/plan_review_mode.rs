use crate::components::plan_review::{OutlinePanel, OutlinePanelMessage, PlanDocument, PlanPanel};
use crate::components::review_comments::{CommentAnchor, ReviewComment};
use std::fmt::Write;
use tui::{Component, Either, Event, Frame, KeyCode, Line, SplitLayout, SplitPanel, Style, ViewContext};

pub enum PlanReviewAction {
    Approve,
    RequestChanges { feedback: String },
    Cancel,
}

pub struct PlanReviewInput {
    pub title: String,
    pub document: PlanDocument,
}

pub struct PlanReviewMode {
    title: String,
    split: SplitPanel<OutlinePanel, PlanPanel>,
}

impl PlanReviewMode {
    pub fn new(input: PlanReviewInput) -> Self {
        let PlanReviewInput { title, document } = input;
        let outline_panel = OutlinePanel::new(document.outline.clone());
        let plan_panel = PlanPanel::new(document);
        let mut split = SplitPanel::new(outline_panel, plan_panel, SplitLayout::fraction(1, 4, 20, 32))
            .with_separator(" ", Style::default())
            .with_resize_keys();

        split.focus_right();
        Self { title, split }
    }

    pub fn current_anchor_line_no(&self) -> usize {
        self.split.right().current_anchor_line_no()
    }

    pub fn comment_count(&self) -> usize {
        self.split.right().comment_count()
    }
}

impl Component for PlanReviewMode {
    type Message = PlanReviewAction;

    async fn on_event(&mut self, event: &Event) -> Option<Vec<Self::Message>> {
        if let Event::Key(key) = event
            && !self.split.right().is_in_comment_mode()
        {
            match key.code {
                KeyCode::Esc => return Some(vec![PlanReviewAction::Cancel]),
                KeyCode::Char('a') => return Some(vec![PlanReviewAction::Approve]),
                KeyCode::Char('r') => {
                    let plan_panel = self.split.right();
                    let feedback = compile_feedback(plan_panel.document(), plan_panel.comments());
                    return Some(vec![PlanReviewAction::RequestChanges { feedback }]);
                }
                KeyCode::Char('u') => {
                    self.split.right_mut().undo_last_comment();
                    return Some(vec![]);
                }
                KeyCode::Char('n') => {
                    self.split.right_mut().jump_next_heading();
                    return Some(vec![]);
                }
                KeyCode::Char('p') => {
                    self.split.right_mut().jump_prev_heading();
                    return Some(vec![]);
                }
                KeyCode::Char('h') | KeyCode::Left if !self.split.is_left_focused() => {
                    self.split.focus_left();
                    return Some(vec![]);
                }
                KeyCode::Char('l') | KeyCode::Right if self.split.is_left_focused() => {
                    self.split.focus_right();
                    return Some(vec![]);
                }
                _ => {}
            }
        }

        let split_messages = self.split.on_event(event).await?;
        for message in split_messages {
            match message {
                Either::Left(OutlinePanelMessage::OpenSelectedAnchor(anchor_line_no)) => {
                    self.split.right_mut().set_cursor_anchor_line_no(anchor_line_no);
                    self.split.focus_right();
                }
                Either::Right(message) => match message {},
            }
        }

        Some(vec![])
    }

    fn render(&mut self, ctx: &ViewContext) -> Frame {
        if ctx.size.width < 20 {
            return Frame::new(vec![Line::new("Plan review view is too narrow")]);
        }

        let mut header = Line::default();
        header.push_with_style(self.title.as_str(), Style::fg(ctx.theme.text_primary()).bold());

        let mut help = Line::default();
        help.push_with_style(
            "j/k:move  g/G:top/bottom  h/l:focus  enter:jump  c:comment  u:undo  a:approve  r:request changes  Esc:cancel",
            Style::fg(ctx.theme.muted()),
        );

        let body_height = ctx.size.height.saturating_sub(2);
        let body_context = ctx.with_size((ctx.size.width, body_height));

        self.split.set_separator_style(Style::default().bg_color(ctx.theme.background()));
        let body = self.split.render(&body_context);

        Frame::vstack([Frame::new(vec![header]), body, Frame::new(vec![help])])
    }
}

fn compile_feedback(document: &PlanDocument, comments: &[ReviewComment<usize>]) -> String {
    if comments.is_empty() {
        return "Plan needs changes, but no inline comments were provided.".to_string();
    }

    let mut output = String::from("# Plan review feedback\n\n");
    let mut current_section: Option<usize> = None;

    for comment in comments {
        let CommentAnchor(line_no) = comment.anchor;
        let Some(line) = document.line_by_no(line_no) else {
            continue;
        };

        if line.section_index != current_section {
            if let Some(section_title) = document.section_title_for(line) {
                let _ = writeln!(output, "## {section_title}");
                output.push('\n');
            }
            current_section = line.section_index;
        }

        let _ = writeln!(output, "### Line {}", line.line_no);
        if !line.text.trim().is_empty() {
            let _ = writeln!(output, "`{}`", sanitize_line_snippet(&line.text));
        }

        let mut wrote_point = false;
        for feedback_line in comment.body.lines().map(str::trim).filter(|line| !line.is_empty()) {
            let _ = writeln!(output, "- {feedback_line}");
            wrote_point = true;
        }

        if !wrote_point {
            output.push_str("- (no comment text provided)\n");
        }

        output.push('\n');
    }

    if output.trim() == "# Plan review feedback" {
        "Plan needs changes, but no inline comments were provided.".to_string()
    } else {
        output.trim().to_string()
    }
}

fn sanitize_line_snippet(line: &str) -> String {
    let mut trimmed = line.trim().replace('`', "\\`");
    if trimmed.chars().count() > 140 {
        trimmed = trimmed.chars().take(137).collect::<String>() + "...";
    }
    trimmed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compile_feedback_falls_back_when_no_comments() {
        let document = PlanDocument::parse("/tmp/plan.md", "# Plan");
        let feedback = compile_feedback(&document, &[]);
        assert!(feedback.contains("no inline comments"));
    }

    #[test]
    fn compile_feedback_includes_line_numbers_and_comments() {
        let document = PlanDocument::parse("/tmp/plan.md", "# Overview\nline");
        let comments = vec![ReviewComment::new(CommentAnchor(2), "Please expand this")];

        let feedback = compile_feedback(&document, &comments);
        assert!(feedback.contains("Line 2"));
        assert!(feedback.contains("Please expand this"));
    }
}
