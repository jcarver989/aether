use crate::components::file_tree::{FileTreeEntry, FileTreeEntryKind};
use crate::git_diff::{FileDiff, FileStatus};
use tui::{Line, Style, truncate_text};

pub(crate) fn render_file_list_cell(
    line: &mut Line,
    files: &[FileDiff],
    row: usize,
    selected: usize,
    left_width: usize,
    theme: &tui::Theme,
) {
    if row >= files.len() {
        line.push_with_style(" ".repeat(left_width), Style::default().bg_color(theme.sidebar_bg()));
        return;
    }

    let file = &files[row];
    let is_selected = row == selected;
    let row_style = row_style(is_selected, theme);
    let marker = if is_selected { "> " } else { "  " };

    line.push_with_style(marker, row_style);
    push_status_marker(line, file.status, is_selected, theme);

    let additions = file.additions();
    let deletions = file.deletions();
    let stats_str = format!("+{additions}/-{deletions}");
    let name_budget = left_width.saturating_sub(4 + stats_str.len() + 1);
    let truncated_path = truncate_text(&file.path, name_budget);

    push_name_padding_stats(
        line,
        truncated_path.as_ref(),
        row_style,
        &stats_str,
        additions,
        deletions,
        left_width.saturating_sub(4),
        is_selected,
        theme,
    );
}

pub(crate) fn render_file_tree_cell(
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
        FileStatus::Added => theme.diff_added_fg(),
        FileStatus::Deleted | FileStatus::Renamed => theme.diff_removed_fg(),
        FileStatus::Modified => theme.text_secondary(),
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
