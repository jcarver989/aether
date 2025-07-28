# Task 04: Diff Generation and Rendering

## Overview
Implement diff generation using the `similar` crate and create reusable diff rendering components for ratatui.

## Dependencies
- Task 02: Core Hook Infrastructure must be completed
- Task 03: Permission Hook Implementation must be completed

## Deliverables

### 1. Add Dependencies to `Cargo.toml`

```toml
[dependencies]
# ... existing dependencies ...
similar = { version = "2.5", features = ["text", "bytes"] }
```

### 2. Diff Generation Module (`src/hooks/diff.rs`)

Create a module for diff generation utilities:

```rust
use similar::{ChangeTag, TextDiff};
use std::fmt::Write;

/// Generate a unified diff between two text strings
pub fn generate_unified_diff(old_content: &str, new_content: &str, filename: &str) -> String {
    let diff = TextDiff::from_lines(old_content, new_content);
    let mut output = String::new();
    
    // Add file headers
    writeln!(&mut output, "--- a/{}", filename).unwrap();
    writeln!(&mut output, "+++ b/{}", filename).unwrap();
    
    // Generate hunks with 3 lines of context
    for (idx, group) in diff.grouped_ops(3).iter().enumerate() {
        if idx > 0 {
            writeln!(&mut output, "...").unwrap();
        }
        
        for op in group {
            let (old_start, old_len) = (op.old_range().start + 1, op.old_range().len());
            let (new_start, new_len) = (op.new_range().start + 1, op.new_range().len());
            
            writeln!(
                &mut output,
                "@@ -{},{} +{},{} @@",
                old_start, old_len, new_start, new_len
            ).unwrap();
            
            for change in diff.iter_changes(op) {
                let sign = match change.tag() {
                    ChangeTag::Delete => "-",
                    ChangeTag::Insert => "+",
                    ChangeTag::Equal => " ",
                };
                
                write!(&mut output, "{}", sign).unwrap();
                
                // Handle missing newline at end of file
                if change.missing_newline() {
                    writeln!(&mut output, "{}\n\\ No newline at end of file", change.value()).unwrap();
                } else {
                    write!(&mut output, "{}", change.value()).unwrap();
                }
            }
        }
    }
    
    output
}

/// Generate a summary of changes (e.g., "+10 -5")
pub fn generate_diff_summary(old_content: &str, new_content: &str) -> DiffSummary {
    let diff = TextDiff::from_lines(old_content, new_content);
    let mut added = 0;
    let mut removed = 0;
    
    for change in diff.iter_all_changes() {
        match change.tag() {
            ChangeTag::Insert => added += 1,
            ChangeTag::Delete => removed += 1,
            ChangeTag::Equal => {}
        }
    }
    
    DiffSummary { added, removed }
}

#[derive(Debug, Clone)]
pub struct DiffSummary {
    pub added: usize,
    pub removed: usize,
}

impl std::fmt::Display for DiffSummary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "+{} -{}", self.added, self.removed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_unified_diff_generation() {
        let old = "line1\nline2\nline3";
        let new = "line1\nmodified line2\nline3\nline4";
        
        let diff = generate_unified_diff(old, new, "test.txt");
        
        assert!(diff.contains("--- a/test.txt"));
        assert!(diff.contains("+++ b/test.txt"));
        assert!(diff.contains("-line2"));
        assert!(diff.contains("+modified line2"));
        assert!(diff.contains("+line4"));
    }
    
    #[test]
    fn test_diff_summary() {
        let old = "line1\nline2\nline3";
        let new = "line1\nmodified line2\nline3\nline4";
        
        let summary = generate_diff_summary(old, new);
        assert_eq!(summary.added, 2);
        assert_eq!(summary.removed, 1);
        assert_eq!(summary.to_string(), "+2 -1");
    }
}
```

### 3. Update Permission Hook

Update `src/hooks/permission.rs` to use the real diff generation:

```rust
use crate::hooks::diff::generate_unified_diff;

impl PermissionHook {
    // Replace the placeholder generate_diff method with:
    fn generate_diff(&self, old: &str, new: &str, filename: &str) -> String {
        generate_unified_diff(old, new, filename)
    }
}
```

### 4. Diff Rendering Component (`src/components/diff_view.rs`)

Create a reusable diff rendering component:

```rust
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
};

#[derive(Debug, Clone)]
pub struct DiffView {
    diff_lines: Vec<DiffLine>,
    scroll_offset: usize,
    file_path: Option<String>,
}

#[derive(Debug, Clone)]
struct DiffLine {
    content: String,
    line_type: DiffLineType,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum DiffLineType {
    Added,
    Removed,
    Context,
    Header,
    HunkHeader,
}

impl DiffView {
    pub fn new(diff_content: &str) -> Self {
        Self {
            diff_lines: Self::parse_diff(diff_content),
            scroll_offset: 0,
            file_path: None,
        }
    }
    
    pub fn with_file_path(mut self, path: String) -> Self {
        self.file_path = Some(path);
        self
    }
    
    fn parse_diff(diff: &str) -> Vec<DiffLine> {
        diff.lines().map(|line| {
            let line_type = if line.starts_with("+++") || line.starts_with("---") {
                DiffLineType::Header
            } else if line.starts_with("@@") {
                DiffLineType::HunkHeader
            } else if line.starts_with("+") {
                DiffLineType::Added
            } else if line.starts_with("-") {
                DiffLineType::Removed
            } else {
                DiffLineType::Context
            };
            
            DiffLine {
                content: line.to_string(),
                line_type,
            }
        }).collect()
    }
    
    pub fn scroll_up(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_sub(1);
    }
    
    pub fn scroll_down(&mut self) {
        let max_scroll = self.diff_lines.len().saturating_sub(10);
        if self.scroll_offset < max_scroll {
            self.scroll_offset += 1;
        }
    }
    
    pub fn render(&self, area: Rect, buf: &mut Buffer) {
        let mut lines: Vec<Line> = Vec::new();
        
        // Add file path if available
        if let Some(path) = &self.file_path {
            lines.push(Line::from(Span::styled(
                path,
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            )));
            lines.push(Line::from(""));
        }
        
        // Render diff lines with syntax highlighting
        for diff_line in self.diff_lines.iter().skip(self.scroll_offset) {
            let (style, prefix_style) = match diff_line.line_type {
                DiffLineType::Added => (
                    Style::default().fg(Color::Green),
                    Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
                ),
                DiffLineType::Removed => (
                    Style::default().fg(Color::Red),
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                ),
                DiffLineType::Header => (
                    Style::default().fg(Color::Blue),
                    Style::default(),
                ),
                DiffLineType::HunkHeader => (
                    Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
                    Style::default(),
                ),
                DiffLineType::Context => (
                    Style::default().fg(Color::Gray),
                    Style::default(),
                ),
            };
            
            // Format line with proper spacing
            let formatted_line = if matches!(diff_line.line_type, DiffLineType::Added | DiffLineType::Removed | DiffLineType::Context) {
                // Add extra space after +/- for alignment
                if let Some(first_char) = diff_line.content.chars().next() {
                    let rest = &diff_line.content[1..];
                    vec![
                        Span::styled(first_char.to_string(), prefix_style),
                        Span::styled(format!(" {}", rest), style),
                    ]
                } else {
                    vec![Span::styled(&diff_line.content, style)]
                }
            } else {
                vec![Span::styled(&diff_line.content, style)]
            };
            
            lines.push(Line::from(formatted_line));
        }
        
        // Add scroll indicator if needed
        if self.diff_lines.len() > area.height as usize {
            let scroll_percentage = (self.scroll_offset as f32 / self.diff_lines.len() as f32 * 100.0) as u16;
            let scroll_indicator = format!("{}% ↕", scroll_percentage);
            
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                scroll_indicator,
                Style::default().fg(Color::DarkGray),
            )));
        }
        
        let paragraph = Paragraph::new(lines)
            .block(Block::default()
                .title("Changes")
                .borders(Borders::ALL))
            .wrap(Wrap { trim: false });
            
        paragraph.render(area, buf);
    }
}

// Make it usable as a regular component
use crate::components::Component;
use crate::action::Action;
use color_eyre::Result;
use crossterm::event::KeyEvent;

impl Component for DiffView {
    fn handle_key_event(&mut self, key: KeyEvent) -> Result<Option<Action>> {
        use crossterm::event::KeyCode;
        
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                self.scroll_up();
                Ok(None)
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.scroll_down();
                Ok(None)
            }
            _ => Ok(None),
        }
    }
    
    fn render(&mut self, area: Rect, buf: &mut Buffer) {
        self.render(area, buf);
    }
}
```

### 5. Update Module Exports

Update `src/hooks/mod.rs`:
```rust
mod types;
mod context;
mod permission;
mod diff;

pub use types::*;
pub use context::*;
pub use permission::PermissionHook;
pub use diff::{generate_unified_diff, generate_diff_summary, DiffSummary};
```

Update `src/components/mod.rs`:
```rust
// ... existing exports ...
mod diff_view;
pub use diff_view::DiffView;
```

## Testing Requirements

### Integration Test
Create `tests/diff_rendering.rs`:

```rust
use aether::hooks::generate_unified_diff;
use aether::components::DiffView;

#[test]
fn test_diff_view_parsing() {
    let old_content = "line1\nline2\nline3";
    let new_content = "line1\nmodified line2\nline3\nnew line4";
    
    let diff = generate_unified_diff(old_content, new_content, "test.rs");
    let diff_view = DiffView::new(&diff);
    
    // Test that diff view correctly parses the diff
    // (Implementation details depend on making fields testable)
}
```

## Acceptance Criteria

- [ ] `similar` crate is added to dependencies
- [ ] Diff generation produces valid unified diff format
- [ ] Diff summary correctly counts additions and deletions
- [ ] DiffView component correctly parses and colorizes diff lines
- [ ] DiffView supports scrolling for long diffs
- [ ] Colors are appropriate: green for additions, red for deletions
- [ ] Permission hook uses real diff generation instead of placeholder
- [ ] All tests pass

## Notes for Implementation

- Use `similar`'s `TextDiff::from_lines` for line-based diffs
- Group operations with context (typically 3 lines) for readability
- Handle edge cases: empty files, no newline at EOF, binary files
- Consider performance for large files (maybe add size limits)
- The DiffView component should be reusable in other contexts
- Add proper error handling for file read operations
- Consider adding line numbers in the diff view for better context