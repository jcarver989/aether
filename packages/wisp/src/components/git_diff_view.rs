use crate::components::app::{GitDiffLoadState, GitDiffViewState, PatchFocus};
use crate::git_diff::{FileDiff, FileStatus, PatchLineKind};
use crate::tui::soft_wrap::truncate_text;
use crate::tui::span::Span;
use crate::tui::{
    Component, InteractiveComponent, KeyCode, Line, MessageResult, RenderContext, Style, UiEvent,
};

pub enum GitDiffViewMessage {
    Close,
    Refresh,
}

pub struct GitDiffView<'a> {
    pub state: &'a mut GitDiffViewState,
}

impl Component for GitDiffView<'_> {
    fn render(&self, context: &RenderContext) -> Vec<Line> {
        let theme = &context.theme;
        let total_width = context.size.width as usize;
        if total_width < 10 {
            return vec![Line::new("Too narrow")];
        }

        let left_width = (total_width / 3)
            .clamp(20, 28)
            .min(total_width.saturating_sub(4));
        let right_width = total_width.saturating_sub(left_width + 1);
        let available_height = context.size.height as usize;

        match &self.state.load_state {
            GitDiffLoadState::Loading => {
                render_message_layout("Loading...", left_width, available_height, theme)
            }
            GitDiffLoadState::Empty => render_message_layout(
                "No changes in working tree relative to HEAD",
                left_width,
                available_height,
                theme,
            ),
            GitDiffLoadState::Error { message } => {
                let msg = format!("Git diff unavailable: {message}");
                render_message_layout(&msg, left_width, available_height, theme)
            }
            GitDiffLoadState::Ready(doc) if doc.files.is_empty() => render_message_layout(
                "No changes in working tree relative to HEAD",
                left_width,
                available_height,
                theme,
            ),
            GitDiffLoadState::Ready(doc) => render_ready(
                &doc.files,
                self.state.selected_file,
                self.state.patch_scroll,
                &self.state.cached_patch_lines,
                left_width,
                right_width,
                available_height,
                context,
            ),
        }
    }
}

impl InteractiveComponent for GitDiffView<'_> {
    type Message = GitDiffViewMessage;

    fn on_event(&mut self, event: UiEvent) -> MessageResult<Self::Message> {
        let UiEvent::Key(key) = event else {
            return MessageResult::consumed();
        };

        match key.code {
            KeyCode::Esc => MessageResult::message(GitDiffViewMessage::Close),
            KeyCode::Char('r') => MessageResult::message(GitDiffViewMessage::Refresh),
            KeyCode::Char('h') | KeyCode::Left => {
                self.state.set_focus(PatchFocus::FileList);
                MessageResult::consumed()
            }
            KeyCode::Enter | KeyCode::Char('l') | KeyCode::Right => {
                self.state.set_focus(PatchFocus::Patch);
                MessageResult::consumed()
            }
            KeyCode::Char('j') | KeyCode::Down => {
                self.navigate_down();
                MessageResult::consumed()
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.navigate_up();
                MessageResult::consumed()
            }
            KeyCode::Char('g') => {
                self.state.scroll_patch_to_start();
                MessageResult::consumed()
            }
            KeyCode::Char('G') => {
                self.state.scroll_patch_to_end();
                MessageResult::consumed()
            }
            KeyCode::PageDown => {
                self.state.scroll_patch(20);
                MessageResult::consumed()
            }
            KeyCode::PageUp => {
                self.state.scroll_patch(-20);
                MessageResult::consumed()
            }
            KeyCode::Char('n') => {
                self.jump_next_hunk();
                MessageResult::consumed()
            }
            KeyCode::Char('p') => {
                self.jump_prev_hunk();
                MessageResult::consumed()
            }
            _ => MessageResult::consumed(),
        }
    }
}

impl GitDiffView<'_> {
    fn navigate_down(&mut self) {
        match self.state.focus {
            PatchFocus::FileList => {
                self.state.select_relative(1);
            }
            PatchFocus::Patch => {
                self.state.scroll_patch(1);
            }
        }
    }

    fn navigate_up(&mut self) {
        match self.state.focus {
            PatchFocus::FileList => {
                self.state.select_relative(-1);
            }
            PatchFocus::Patch => {
                self.state.scroll_patch(-1);
            }
        }
    }

    fn jump_next_hunk(&mut self) {
        self.state.jump_next_hunk();
    }

    fn jump_prev_hunk(&mut self) {
        self.state.jump_prev_hunk();
    }
}

#[allow(clippy::too_many_arguments)]
fn render_ready(
    files: &[FileDiff],
    selected_file_idx: usize,
    patch_scroll: usize,
    cached_patch_lines: &[Line],
    left_width: usize,
    right_width: usize,
    available_height: usize,
    context: &RenderContext,
) -> Vec<Line> {
    let theme = &context.theme;
    let selected = selected_file_idx.min(files.len().saturating_sub(1));
    let selected_file = &files[selected];

    let row_count = available_height.max(files.len());
    let mut rows = Vec::with_capacity(row_count);

    for i in 0..row_count {
        let mut line = Line::default();

        render_file_list_cell(&mut line, files, i, selected, left_width, theme);
        line.push_with_style("\u{2502}", Style::fg(theme.text_secondary()));
        render_patch_cell(
            &mut line,
            selected_file,
            cached_patch_lines,
            i,
            patch_scroll,
            right_width,
            theme,
        );

        rows.push(line);
    }

    rows
}

fn render_file_list_cell(
    line: &mut Line,
    files: &[FileDiff],
    row: usize,
    selected: usize,
    left_width: usize,
    theme: &crate::tui::Theme,
) {
    if row >= files.len() {
        line.push_text(" ".repeat(left_width));
        return;
    }

    let file = &files[row];
    let is_selected = row == selected;
    let marker = if is_selected { "> " } else { "  " };
    let status_char = file.status.marker();
    let status_color = match file.status {
        FileStatus::Added => theme.diff_added_fg(),
        FileStatus::Deleted | FileStatus::Renamed => theme.diff_removed_fg(),
        FileStatus::Modified => theme.text_secondary(),
    };

    let stats_str = format!("+{}/-{}", file.additions(), file.deletions());
    let stats_width = stats_str.len();
    let path_budget = left_width.saturating_sub(4 + stats_width + 1);
    let truncated_path = truncate_text(&file.path, path_budget);
    let path_width = truncated_path.chars().count();
    let padding = left_width.saturating_sub(4 + path_width + stats_width);

    let row_style = if is_selected {
        theme.selected_row_style()
    } else {
        Style::default()
    };

    line.push_with_style(marker, row_style);
    line.push_with_style(
        format!("{status_char} "),
        if is_selected {
            theme.selected_row_style_with_fg(status_color)
        } else {
            Style::fg(status_color)
        },
    );
    line.push_with_style(truncated_path.as_ref(), row_style);
    if padding > 0 {
        line.push_with_style(" ".repeat(padding), row_style);
    }
    line.push_with_style(
        &stats_str,
        if is_selected {
            theme.selected_row_style_with_fg(theme.text_secondary())
        } else {
            Style::fg(theme.text_secondary())
        },
    );
}

#[allow(clippy::too_many_arguments)]
fn render_patch_cell(
    line: &mut Line,
    selected_file: &FileDiff,
    patch_lines: &[Line],
    row: usize,
    patch_scroll: usize,
    right_width: usize,
    theme: &crate::tui::Theme,
) {
    if row == 0 {
        let header_text = match selected_file.status {
            FileStatus::Renamed => {
                let old = selected_file.old_path.as_deref().unwrap_or("?");
                format!("{old} -> {}", selected_file.path)
            }
            _ => selected_file.path.clone(),
        };
        let status_label = match selected_file.status {
            FileStatus::Modified => "modified",
            FileStatus::Added => "new file",
            FileStatus::Deleted => "deleted",
            FileStatus::Renamed => "renamed",
        };
        let full_header = format!("{header_text}  ({status_label})");
        let truncated = truncate_text(&full_header, right_width);
        line.push_with_style(truncated.as_ref(), Style::default().bold());
    } else if row == 1 {
        // spacer
    } else if selected_file.binary {
        if row == 2 {
            line.push_with_style("Binary file", Style::fg(theme.text_secondary()));
        }
    } else {
        let patch_row = row - 2;
        let scrolled_row = patch_row + patch_scroll;
        if scrolled_row < patch_lines.len() {
            line.append_line(&patch_lines[scrolled_row]);
        }
    }
}

fn render_message_layout(
    message: &str,
    left_width: usize,
    available_height: usize,
    theme: &crate::tui::Theme,
) -> Vec<Line> {
    let mut rows = Vec::with_capacity(available_height);
    for i in 0..available_height {
        let mut line = Line::default();
        line.push_text(" ".repeat(left_width));
        line.push_with_style("\u{2502}", Style::fg(theme.text_secondary()));
        if i == 0 {
            line.push_with_style(message, Style::fg(theme.text_secondary()));
        }
        rows.push(line);
    }
    rows
}

pub(crate) fn build_patch_lines(file: &FileDiff, context: &RenderContext) -> Vec<Line> {
    let theme = &context.theme;
    let lang_hint = lang_hint_from_path(&file.path);
    let mut patch_lines = Vec::new();

    let max_line_no = file
        .hunks
        .iter()
        .flat_map(|h| &h.lines)
        .filter_map(|l| l.old_line_no.into_iter().chain(l.new_line_no).max())
        .max()
        .unwrap_or(0);
    let gutter_width = digit_count(max_line_no);

    for (hunk_idx, hunk) in file.hunks.iter().enumerate() {
        if hunk_idx > 0 {
            patch_lines.push(Line::default());
        }

        for pl in &hunk.lines {
            let mut line = Line::default();

            match pl.kind {
                PatchLineKind::HunkHeader => {
                    line.push_with_style(&pl.text, Style::fg(theme.info()).bold());
                }
                PatchLineKind::Context => {
                    let old_str = format_line_no(pl.old_line_no, gutter_width);
                    let new_str = format_line_no(pl.new_line_no, gutter_width);
                    line.push_with_style(
                        format!("{old_str} {new_str}   "),
                        Style::fg(theme.text_secondary()),
                    );
                    append_syntax_spans(&mut line, &pl.text, lang_hint, None, context);
                }
                PatchLineKind::Added => {
                    let old_str = " ".repeat(gutter_width);
                    let new_str = format_line_no(pl.new_line_no, gutter_width);
                    let bg = Some(theme.diff_added_bg());
                    let style = Style::fg(theme.diff_added_fg()).bg_color(theme.diff_added_bg());
                    line.push_with_style(format!("{old_str} {new_str} + "), style);
                    append_syntax_spans(&mut line, &pl.text, lang_hint, bg, context);
                }
                PatchLineKind::Removed => {
                    let old_str = format_line_no(pl.old_line_no, gutter_width);
                    let new_str = " ".repeat(gutter_width);
                    let bg = Some(theme.diff_removed_bg());
                    let style =
                        Style::fg(theme.diff_removed_fg()).bg_color(theme.diff_removed_bg());
                    line.push_with_style(format!("{old_str} {new_str} - "), style);
                    append_syntax_spans(&mut line, &pl.text, lang_hint, bg, context);
                }
                PatchLineKind::Meta => {
                    line.push_with_style(&pl.text, Style::fg(theme.text_secondary()).italic());
                }
            }

            patch_lines.push(line);
        }
    }

    patch_lines
}

fn lang_hint_from_path(path: &str) -> &str {
    path.rsplit('.').next().unwrap_or("")
}

fn append_syntax_spans(
    line: &mut Line,
    text: &str,
    lang_hint: &str,
    bg_override: Option<crate::tui::Color>,
    context: &RenderContext,
) {
    let spans = context
        .highlighter()
        .highlight(text, lang_hint, &context.theme);
    if let Some(content) = spans.first() {
        for span in content.spans() {
            let mut span_style = span.style();
            if let Some(bg) = bg_override {
                span_style.bg = Some(bg);
            }
            line.push_span(Span::with_style(span.text(), span_style));
        }
    } else {
        line.push_text(text);
    }
}

fn format_line_no(line_no: Option<usize>, width: usize) -> String {
    match line_no {
        Some(n) => format!("{n:>width$}"),
        None => " ".repeat(width),
    }
}

fn digit_count(mut n: usize) -> usize {
    if n == 0 {
        return 1;
    }
    let mut count = 0;
    while n > 0 {
        count += 1;
        n /= 10;
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git_diff::{FileDiff, FileStatus, GitDiffDocument, Hunk, PatchLine, PatchLineKind};
    use crate::tui::KeyEvent;
    use crate::tui::KeyModifiers;
    use std::path::PathBuf;

    fn make_test_doc() -> GitDiffDocument {
        GitDiffDocument {
            repo_root: PathBuf::from("/tmp/test"),
            files: vec![
                FileDiff {
                    old_path: Some("a.rs".to_string()),
                    path: "a.rs".to_string(),
                    status: FileStatus::Modified,
                    hunks: vec![Hunk {
                        header: "@@ -1,3 +1,3 @@".to_string(),
                        old_start: 1,
                        old_count: 3,
                        new_start: 1,
                        new_count: 3,
                        lines: vec![
                            PatchLine {
                                kind: PatchLineKind::HunkHeader,
                                text: "@@ -1,3 +1,3 @@".to_string(),
                                old_line_no: None,
                                new_line_no: None,
                            },
                            PatchLine {
                                kind: PatchLineKind::Context,
                                text: "fn main() {".to_string(),
                                old_line_no: Some(1),
                                new_line_no: Some(1),
                            },
                            PatchLine {
                                kind: PatchLineKind::Removed,
                                text: "    old();".to_string(),
                                old_line_no: Some(2),
                                new_line_no: None,
                            },
                            PatchLine {
                                kind: PatchLineKind::Added,
                                text: "    new();".to_string(),
                                old_line_no: None,
                                new_line_no: Some(2),
                            },
                            PatchLine {
                                kind: PatchLineKind::Context,
                                text: "}".to_string(),
                                old_line_no: Some(3),
                                new_line_no: Some(3),
                            },
                        ],
                    }],
                    binary: false,
                },
                FileDiff {
                    old_path: None,
                    path: "b.rs".to_string(),
                    status: FileStatus::Added,
                    hunks: vec![Hunk {
                        header: "@@ -0,0 +1,1 @@".to_string(),
                        old_start: 0,
                        old_count: 0,
                        new_start: 1,
                        new_count: 1,
                        lines: vec![
                            PatchLine {
                                kind: PatchLineKind::HunkHeader,
                                text: "@@ -0,0 +1,1 @@".to_string(),
                                old_line_no: None,
                                new_line_no: None,
                            },
                            PatchLine {
                                kind: PatchLineKind::Added,
                                text: "new_content".to_string(),
                                old_line_no: None,
                                new_line_no: Some(1),
                            },
                        ],
                    }],
                    binary: false,
                },
            ],
        }
    }

    fn make_view_state(doc: GitDiffDocument) -> GitDiffViewState {
        GitDiffViewState::new(GitDiffLoadState::Ready(doc))
    }

    #[test]
    fn esc_emits_close() {
        let doc = make_test_doc();
        let mut state = make_view_state(doc);
        let mut view = GitDiffView { state: &mut state };
        let result = view.on_event(UiEvent::Key(KeyEvent::new(
            KeyCode::Esc,
            KeyModifiers::NONE,
        )));
        assert!(
            result
                .messages
                .iter()
                .any(|m| matches!(m, GitDiffViewMessage::Close))
        );
    }

    #[test]
    fn r_emits_refresh() {
        let doc = make_test_doc();
        let mut state = make_view_state(doc);
        let mut view = GitDiffView { state: &mut state };
        let result = view.on_event(UiEvent::Key(KeyEvent::new(
            KeyCode::Char('r'),
            KeyModifiers::NONE,
        )));
        assert!(
            result
                .messages
                .iter()
                .any(|m| matches!(m, GitDiffViewMessage::Refresh))
        );
    }

    #[test]
    fn j_moves_file_selection_down() {
        let doc = make_test_doc();
        let mut state = make_view_state(doc);
        assert_eq!(state.selected_file, 0);

        let mut view = GitDiffView { state: &mut state };
        view.on_event(UiEvent::Key(KeyEvent::new(
            KeyCode::Char('j'),
            KeyModifiers::NONE,
        )));
        assert_eq!(view.state.selected_file, 1);
    }

    #[test]
    fn k_moves_file_selection_up_with_wrap() {
        let doc = make_test_doc();
        let mut state = make_view_state(doc);
        assert_eq!(state.selected_file, 0);

        let mut view = GitDiffView { state: &mut state };
        view.on_event(UiEvent::Key(KeyEvent::new(
            KeyCode::Char('k'),
            KeyModifiers::NONE,
        )));
        assert_eq!(view.state.selected_file, 1); // wraps from 0 to last
    }

    #[test]
    fn enter_switches_to_patch_focus() {
        let doc = make_test_doc();
        let mut state = make_view_state(doc);
        assert_eq!(state.focus, PatchFocus::FileList);

        let mut view = GitDiffView { state: &mut state };
        view.on_event(UiEvent::Key(KeyEvent::new(
            KeyCode::Enter,
            KeyModifiers::NONE,
        )));
        assert_eq!(view.state.focus, PatchFocus::Patch);
    }

    #[test]
    fn h_switches_to_file_list_focus() {
        let doc = make_test_doc();
        let mut state = make_view_state(doc);
        state.focus = PatchFocus::Patch;

        let mut view = GitDiffView { state: &mut state };
        view.on_event(UiEvent::Key(KeyEvent::new(
            KeyCode::Char('h'),
            KeyModifiers::NONE,
        )));
        assert_eq!(view.state.focus, PatchFocus::FileList);
    }

    #[test]
    fn file_selection_resets_patch_scroll() {
        let doc = make_test_doc();
        let mut state = make_view_state(doc);
        state.patch_scroll = 5;

        let mut view = GitDiffView { state: &mut state };
        view.on_event(UiEvent::Key(KeyEvent::new(
            KeyCode::Char('j'),
            KeyModifiers::NONE,
        )));
        assert_eq!(view.state.patch_scroll, 0);
    }

    #[test]
    fn digit_count_works() {
        assert_eq!(digit_count(0), 1);
        assert_eq!(digit_count(1), 1);
        assert_eq!(digit_count(9), 1);
        assert_eq!(digit_count(10), 2);
        assert_eq!(digit_count(99), 2);
        assert_eq!(digit_count(100), 3);
        assert_eq!(digit_count(999), 3);
    }

    #[test]
    fn render_empty_state() {
        let mut state = GitDiffViewState::new(GitDiffLoadState::Empty);
        let view = GitDiffView { state: &mut state };
        let context = RenderContext::new((80, 24));
        let lines = view.render(&context);
        let text: String = lines.iter().map(|l| l.plain_text()).collect();
        assert!(text.contains("No changes"));
    }

    #[test]
    fn render_error_state() {
        let mut state = GitDiffViewState::new(GitDiffLoadState::Error {
            message: "not a repo".to_string(),
        });
        let view = GitDiffView { state: &mut state };
        let context = RenderContext::new((80, 24));
        let lines = view.render(&context);
        let text: String = lines.iter().map(|l| l.plain_text()).collect();
        assert!(text.contains("Git diff unavailable"));
        assert!(text.contains("not a repo"));
    }

    #[test]
    fn render_shows_file_list_and_patch() {
        let doc = make_test_doc();
        let mut state = make_view_state(doc);
        let view = GitDiffView { state: &mut state };
        let context = RenderContext::new((100, 24));
        let lines = view.render(&context);
        assert!(!lines.is_empty());

        // First line should have file list entry and file header
        let first_text = lines[0].plain_text();
        assert!(
            first_text.contains("a.rs"),
            "Should show file name: {first_text}"
        );
    }

    #[test]
    fn patch_lines_have_syntax_highlighted_spans() {
        let doc = make_test_doc();
        let context = RenderContext::new((100, 24));
        let file = &doc.files[0];
        let patch_lines = build_patch_lines(file, &context);

        // Context line "fn main() {" should have multiple spans from syntax highlighting
        // (gutter span + syntax spans for "fn", "main", etc.)
        let context_line = &patch_lines[1]; // index 1 = first Context line after HunkHeader
        assert!(
            context_line.spans().len() > 2,
            "Expected syntax-highlighted spans for context line, got {} spans",
            context_line.spans().len()
        );

        // Added line should also have syntax spans with bg overlay
        let added_line = &patch_lines[3]; // Added line
        let content_spans: Vec<_> = added_line.spans().iter().skip(1).collect(); // skip gutter
        assert!(
            !content_spans.is_empty(),
            "Added line should have content spans"
        );
        // All content spans should have the added bg color
        let theme = &context.theme;
        for span in &content_spans {
            assert_eq!(
                span.style().bg,
                Some(theme.diff_added_bg()),
                "Added line spans should have diff_added_bg"
            );
        }
    }

    #[test]
    fn lang_hint_extracts_extension() {
        assert_eq!(lang_hint_from_path("src/main.rs"), "rs");
        assert_eq!(lang_hint_from_path("foo.py"), "py");
        assert_eq!(lang_hint_from_path("Makefile"), "Makefile");
        assert_eq!(lang_hint_from_path("a/b/c.tsx"), "tsx");
    }
}
