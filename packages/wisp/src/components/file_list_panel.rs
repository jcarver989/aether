use crate::components::file_tree::{FileTree, FileTreeEntry, FileTreeEntryKind};
use crate::git_diff::FileStatus;
use tui::{Component, Event, Frame, KeyCode, Line, Style, ViewContext, truncate_text, wrap_selection};

pub struct FileListPanel {
    pub(crate) file_count: usize,
    pub(crate) selected: usize,
    pub(crate) scroll: usize,
    pub(crate) tree: Option<FileTree>,
    pub(crate) queued_comment_count: usize,
}

pub enum FileListMessage {
    Selected(usize),
}

impl FileListPanel {
    pub fn new() -> Self {
        Self { file_count: 0, selected: 0, scroll: 0, tree: None, queued_comment_count: 0 }
    }

    pub fn with_tree(mut self, tree: FileTree) -> Self {
        self.tree = Some(tree);
        self
    }

    pub fn with_queued_comment_count(mut self, count: usize) -> Self {
        self.queued_comment_count = count;
        self
    }

    pub(crate) fn select_relative(&mut self, delta: isize) -> Option<usize> {
        if let Some(tree) = &mut self.tree {
            let prev_file = tree.selected_file_index();
            tree.navigate(delta);
            let new_file = tree.selected_file_index();
            if let Some(idx) = new_file
                && Some(idx) != prev_file.or(Some(self.selected))
            {
                self.selected = idx;
                return Some(idx);
            }
            return None;
        }

        if self.file_count == 0 {
            return None;
        }

        let previous = self.selected;
        wrap_selection(&mut self.selected, self.file_count, delta);
        if self.selected == previous { None } else { Some(self.selected) }
    }

    pub(crate) fn tree_collapse_or_parent(&mut self) {
        if let Some(tree) = &mut self.tree {
            tree.collapse_or_parent();
        }
    }

    pub(crate) fn tree_expand_or_enter(&mut self) -> Option<usize> {
        let Some(tree) = &mut self.tree else {
            return Some(self.selected);
        };
        let is_file = tree.expand_or_enter();
        if is_file {
            if let Some(idx) = tree.selected_file_index()
                && idx != self.selected
            {
                self.selected = idx;
            }
            Some(self.selected)
        } else {
            None
        }
    }

    pub(crate) fn ensure_visible(&mut self, viewport_height: usize) {
        let Some(tree) = &self.tree else {
            return;
        };
        let selected = tree.selected_visible();
        if selected < self.scroll {
            self.scroll = selected;
        } else if selected >= self.scroll + viewport_height {
            self.scroll = selected.saturating_sub(viewport_height - 1);
        }
    }
}

impl Component for FileListPanel {
    type Message = FileListMessage;

    async fn on_event(&mut self, event: &Event) -> Option<Vec<Self::Message>> {
        let Event::Key(key) = event else {
            return None;
        };
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => {
                let msgs = self.select_relative(1).map(|idx| vec![FileListMessage::Selected(idx)]).unwrap_or_default();
                Some(msgs)
            }
            KeyCode::Char('k') | KeyCode::Up => {
                let msgs = self.select_relative(-1).map(|idx| vec![FileListMessage::Selected(idx)]).unwrap_or_default();
                Some(msgs)
            }
            KeyCode::Char('h') | KeyCode::Left => {
                self.tree_collapse_or_parent();
                Some(vec![])
            }
            KeyCode::Enter | KeyCode::Char('l') | KeyCode::Right => {
                if let Some(idx) = self.tree_expand_or_enter() {
                    Some(vec![FileListMessage::Selected(idx)])
                } else {
                    Some(vec![])
                }
            }
            _ => None,
        }
    }

    fn render(&mut self, ctx: &ViewContext) -> Frame {
        let theme = &ctx.theme;
        let width = ctx.size.width as usize;
        let height = ctx.size.height as usize;

        if let Some(tree) = &mut self.tree {
            tree.ensure_cache();
        }

        let visible_entries = self.tree.as_ref().map(FileTree::visible_entries).unwrap_or_default();
        let tree_selected = self.tree.as_ref().map_or(0, FileTree::selected_visible);

        let entry_count = if visible_entries.is_empty() { self.file_count } else { visible_entries.len() };
        let row_count = height.max(entry_count);
        let mut lines = Vec::with_capacity(height);

        for i in 0..row_count {
            let mut line = Line::default();
            let queue_row = self.queued_comment_count > 0 && i == height.saturating_sub(1);
            if queue_row {
                let indicator = format!(
                    " [{} comment{}] s:submit u:undo",
                    self.queued_comment_count,
                    if self.queued_comment_count == 1 { "" } else { "s" },
                );
                let padded = truncate_text(&indicator, width);
                let pad = width.saturating_sub(padded.chars().count());
                line.push_with_style(padded.as_ref(), Style::fg(theme.accent()).bg_color(theme.sidebar_bg()));
                if pad > 0 {
                    line.push_with_style(" ".repeat(pad), Style::default().bg_color(theme.sidebar_bg()));
                }
            } else if !visible_entries.is_empty() {
                let scrolled_i = i + self.scroll;
                if let Some(entry) = visible_entries.get(scrolled_i) {
                    render_file_tree_cell(&mut line, entry, scrolled_i == tree_selected, width, theme);
                } else {
                    line.push_with_style(" ".repeat(width), Style::default().bg_color(theme.sidebar_bg()));
                }
            } else {
                line.push_with_style(" ".repeat(width), Style::default().bg_color(theme.sidebar_bg()));
            }

            lines.push(line);
        }

        lines.truncate(height);
        Frame::new(lines)
    }
}

fn render_file_tree_cell(
    line: &mut Line,
    entry: &FileTreeEntry,
    is_selected: bool,
    left_width: usize,
    theme: &tui::Theme,
) {
    let style = row_style(is_selected, theme);
    let marker = if is_selected { "> " } else { "  " };
    let indent = "  ".repeat(entry.depth);
    let prefix_width = 2 + entry.depth * 2 + 2;

    match &entry.kind {
        FileTreeEntryKind::Directory { name, expanded, .. } => {
            let icon = if *expanded { "\u{25be} " } else { "\u{25b8} " };
            let name_budget = left_width.saturating_sub(prefix_width);
            let display_name = format!("{name}/");
            let truncated = truncate_text(&display_name, name_budget);
            let remaining = left_width.saturating_sub(prefix_width + truncated.chars().count());

            line.push_with_style(format!("{marker}{indent}{icon}"), style);
            line.push_with_style(truncated.as_ref(), style.bold());
            if remaining > 0 {
                line.push_with_style(" ".repeat(remaining), style);
            }
        }
        FileTreeEntryKind::File { name, status, additions, deletions, .. } => {
            let stats_str = format!("+{additions}/-{deletions}");
            let name_budget = left_width.saturating_sub(prefix_width + 2 + stats_str.len() + 1);
            let truncated = truncate_text(name, name_budget);

            line.push_with_style(format!("{marker}{indent}  "), style);
            push_status_marker(line, *status, is_selected, theme);
            push_name_padding_stats(
                line,
                truncated.as_ref(),
                style,
                &stats_str,
                *additions,
                *deletions,
                left_width.saturating_sub(prefix_width + 2),
                is_selected,
                theme,
            );
        }
    }
}

fn row_style(is_selected: bool, theme: &tui::Theme) -> Style {
    if is_selected { theme.selected_row_style() } else { Style::default().bg_color(theme.sidebar_bg()) }
}

fn push_status_marker(line: &mut Line, status: FileStatus, is_selected: bool, theme: &tui::Theme) {
    let status_color = match status {
        FileStatus::Deleted | FileStatus::Renamed => theme.diff_removed_fg(),
        FileStatus::Modified => theme.text_secondary(),
        FileStatus::Added | FileStatus::Untracked => theme.diff_added_fg(),
    };
    line.push_with_style(
        format!("{} ", status.marker()),
        if is_selected {
            theme.selected_row_style_with_fg(status_color)
        } else {
            Style::fg(status_color).bg_color(theme.sidebar_bg())
        },
    );
}

#[allow(clippy::too_many_arguments)]
fn push_name_padding_stats(
    line: &mut Line,
    name: &str,
    name_style: Style,
    stats_str: &str,
    additions: usize,
    deletions: usize,
    available: usize,
    is_selected: bool,
    theme: &tui::Theme,
) {
    let name_width = name.chars().count();
    let padding = available.saturating_sub(name_width + stats_str.len());

    line.push_with_style(name, name_style);
    if padding > 0 {
        line.push_with_style(
            " ".repeat(padding),
            if is_selected { theme.selected_row_style() } else { Style::default().bg_color(theme.sidebar_bg()) },
        );
    }

    let add_str = format!("+{additions}");
    let del_str = format!("/-{deletions}");
    line.push_with_style(
        &add_str,
        if is_selected {
            theme.selected_row_style_with_fg(theme.diff_added_fg())
        } else {
            Style::fg(theme.diff_added_fg()).bg_color(theme.sidebar_bg())
        },
    );
    line.push_with_style(
        &del_str,
        if is_selected {
            theme.selected_row_style_with_fg(theme.diff_removed_fg())
        } else {
            Style::fg(theme.diff_removed_fg()).bg_color(theme.sidebar_bg())
        },
    );
}
