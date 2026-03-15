use crate::git_diff::{FileDiff, FileStatus};
use crate::tui::{Line, Style, truncate_text};

pub(crate) fn render_file_list_cell(
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
