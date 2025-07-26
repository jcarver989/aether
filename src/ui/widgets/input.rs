use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Widget, Wrap},
};

#[derive(Debug, Clone)]
pub struct InputState {
    pub lines: Vec<String>,
    pub cursor_line: usize,
    pub cursor_col: usize,
}

impl InputState {
    pub fn new() -> Self {
        Self {
            lines: vec![String::new()],
            cursor_line: 0,
            cursor_col: 0,
        }
    }

    pub fn from_text(text: &str) -> Self {
        let lines: Vec<String> = text.lines().map(|s| s.to_string()).collect();
        let lines = if lines.is_empty() { vec![String::new()] } else { lines };
        
        Self {
            cursor_line: lines.len().saturating_sub(1),
            cursor_col: lines.last().map(|l| l.len()).unwrap_or(0),
            lines,
        }
    }

    pub fn insert_char(&mut self, ch: char) {
        if self.cursor_line < self.lines.len() {
            let line = &mut self.lines[self.cursor_line];
            if self.cursor_col <= line.len() {
                line.insert(self.cursor_col, ch);
                self.cursor_col += 1;
            }
        }
    }

    pub fn insert_newline(&mut self) {
        if self.cursor_line < self.lines.len() {
            let current_line = self.lines[self.cursor_line].clone();
            let (left, right) = current_line.split_at(self.cursor_col);
            
            self.lines[self.cursor_line] = left.to_string();
            self.lines.insert(self.cursor_line + 1, right.to_string());
            
            self.cursor_line += 1;
            self.cursor_col = 0;
        }
    }

    pub fn delete_char(&mut self) {
        if self.cursor_line < self.lines.len() {
            let line = &mut self.lines[self.cursor_line];
            if self.cursor_col > 0 && self.cursor_col <= line.len() {
                line.remove(self.cursor_col - 1);
                self.cursor_col -= 1;
            } else if self.cursor_col == 0 && self.cursor_line > 0 {
                // Merge with previous line
                let current_line = self.lines.remove(self.cursor_line);
                self.cursor_line -= 1;
                self.cursor_col = self.lines[self.cursor_line].len();
                self.lines[self.cursor_line].push_str(&current_line);
            }
        }
    }

    pub fn move_cursor_left(&mut self) {
        if self.cursor_col > 0 {
            self.cursor_col -= 1;
        } else if self.cursor_line > 0 {
            self.cursor_line -= 1;
            self.cursor_col = self.lines.get(self.cursor_line).map(|l| l.len()).unwrap_or(0);
        }
    }

    pub fn move_cursor_right(&mut self) {
        if self.cursor_line < self.lines.len() {
            let line_len = self.lines[self.cursor_line].len();
            if self.cursor_col < line_len {
                self.cursor_col += 1;
            } else if self.cursor_line + 1 < self.lines.len() {
                self.cursor_line += 1;
                self.cursor_col = 0;
            }
        }
    }

    pub fn move_cursor_up(&mut self) {
        if self.cursor_line > 0 {
            self.cursor_line -= 1;
            let line_len = self.lines.get(self.cursor_line).map(|l| l.len()).unwrap_or(0);
            self.cursor_col = self.cursor_col.min(line_len);
        }
    }

    pub fn move_cursor_down(&mut self) {
        if self.cursor_line + 1 < self.lines.len() {
            self.cursor_line += 1;
            let line_len = self.lines.get(self.cursor_line).map(|l| l.len()).unwrap_or(0);
            self.cursor_col = self.cursor_col.min(line_len);
        }
    }

    pub fn to_string(&self) -> String {
        self.lines.join("\n")
    }

    pub fn is_empty(&self) -> bool {
        self.lines.len() == 1 && self.lines[0].is_empty()
    }

    pub fn clear(&mut self) {
        self.lines = vec![String::new()];
        self.cursor_line = 0;
        self.cursor_col = 0;
    }
}

impl Default for InputState {
    fn default() -> Self {
        Self::new()
    }
}

pub struct InputWidget<'a> {
    state: &'a InputState,
    placeholder: Option<&'a str>,
    show_cursor: bool,
    block: Option<Block<'a>>,
}

impl<'a> InputWidget<'a> {
    pub fn new(state: &'a InputState) -> Self {
        Self {
            state,
            placeholder: None,
            show_cursor: true,
            block: None,
        }
    }

    pub fn placeholder(mut self, placeholder: &'a str) -> Self {
        self.placeholder = Some(placeholder);
        self
    }

    pub fn show_cursor(mut self, show: bool) -> Self {
        self.show_cursor = show;
        self
    }

    pub fn block(mut self, block: Block<'a>) -> Self {
        self.block = Some(block);
        self
    }

    fn format_lines(&self) -> Vec<Line<'a>> {
        if self.state.is_empty() && self.placeholder.is_some() {
            return vec![Line::from(vec![
                Span::styled("> ", Style::default().fg(Color::Green)),
                Span::styled(
                    self.placeholder.unwrap(),
                    Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC),
                ),
            ])];
        }

        let mut lines = Vec::new();
        
        for (line_idx, line_content) in self.state.lines.iter().enumerate() {
            let mut spans = vec![
                if line_idx == 0 {
                    Span::styled("> ", Style::default().fg(Color::Green))
                } else {
                    Span::raw("  ")
                }
            ];

            if line_idx == self.state.cursor_line && self.show_cursor {
                // Split line at cursor position
                let (before_cursor, after_cursor) = line_content.split_at(self.state.cursor_col);
                
                if !before_cursor.is_empty() {
                    spans.push(Span::raw(before_cursor.to_string()));
                }

                // Cursor character (either the next char or a space)
                let cursor_char = after_cursor.chars().next().unwrap_or(' ');
                spans.push(Span::styled(
                    cursor_char.to_string(),
                    Style::default().bg(Color::White).fg(Color::Black),
                ));

                // Rest of the line after cursor
                if after_cursor.len() > 1 {
                    spans.push(Span::raw(after_cursor.chars().skip(1).collect::<String>()));
                }
            } else {
                spans.push(Span::raw(line_content.clone()));
            }

            lines.push(Line::from(spans));
        }

        // Show help text on the last line
        if self.state.lines.len() == 1 && self.state.lines[0].is_empty() {
            lines.push(Line::from(vec![
                Span::styled(
                    "Enter to submit, Shift+Enter for new line, Ctrl+C to cancel",
                    Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC),
                ),
            ]));
        } else if !self.state.is_empty() {
            lines.push(Line::from(vec![
                Span::styled(
                    format!("{} lines | Enter to submit", self.state.lines.len()),
                    Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC),
                ),
            ]));
        }

        lines
    }
}

impl<'a> Widget for InputWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let content = self.format_lines();
        let text = Text::from(content);
        
        let mut paragraph = Paragraph::new(text)
            .wrap(Wrap { trim: false });

        if let Some(block) = self.block {
            paragraph = paragraph.block(block);
        }

        paragraph.render(area, buf);
    }
}