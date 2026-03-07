use pulldown_cmark::{Alignment, CodeBlockKind, Event, Options, Parser, Tag, TagEnd};
use std::borrow::Cow;
use std::collections::HashMap;
use syntect::easy::HighlightLines;
use unicode_width::UnicodeWidthStr;

use super::screen::{Line, Span, Style};
use super::syntax::{find_syntax_by_token, syntax_set, syntect_to_wisp_style};
use super::theme::Theme;

/// A single rendered cell in a table row.
#[derive(Clone, Debug)]
struct TableCell {
    /// Styled content lines.
    lines: Vec<Line>,
    /// Horizontal alignment for this cell.
    alignment: Alignment,
    /// Maximum display width across `lines`.
    max_width: usize,
}

impl Default for TableCell {
    fn default() -> Self {
        Self {
            lines: Vec::new(),
            alignment: Alignment::None,
            max_width: 0,
        }
    }
}

/// A row in a table.
type TableRow = Vec<TableCell>;

/// Builds styled inline content for a single table cell.
#[derive(Clone, Debug, Default)]
struct CellBuilder {
    lines: Vec<Line>,
    current_line: Line,
}

impl CellBuilder {
    fn push_text(&mut self, text: &str, style: Style) {
        for (i, chunk) in text.split('\n').enumerate() {
            if i > 0 {
                self.flush_line();
            }
            if !chunk.is_empty() {
                self.current_line.push_span(Span::with_style(chunk, style));
            }
        }
    }

    fn push_code(&mut self, code: &str, style: Style) {
        if !code.is_empty() {
            self.current_line.push_span(Span::with_style(code, style));
        }
    }

    fn soft_break(&mut self, style: Style) {
        self.current_line.push_span(Span::with_style(" ", style));
    }

    fn hard_break(&mut self) {
        self.flush_line();
    }

    fn finish(mut self) -> Vec<Line> {
        if !self.current_line.is_empty() || !self.lines.is_empty() {
            self.lines.push(std::mem::take(&mut self.current_line));
        }
        self.lines
    }

    fn flush_line(&mut self) {
        let line = std::mem::take(&mut self.current_line);
        self.lines.push(line);
    }
}

/// Manages table state during parsing and rendering.
#[derive(Clone, Debug, Default)]
struct TableState {
    /// Column alignments from markdown table syntax.
    alignments: Vec<Alignment>,
    /// All rows in the table (including header).
    rows: Vec<TableRow>,
    /// Current row being built.
    current_row: Vec<TableCell>,
    /// Display width for each column, including left/right padding.
    column_widths: Vec<usize>,
}

impl TableState {
    fn new(alignments: &[Alignment]) -> Self {
        Self {
            alignments: alignments.to_vec(),
            rows: Vec::new(),
            current_row: Vec::new(),
            column_widths: vec![0; alignments.len()],
        }
    }

    fn start_row(&mut self) {
        self.current_row.clear();
    }

    fn add_cell(&mut self, cell: TableCell) {
        let col_idx = self.current_row.len();
        let needed = cell.max_width + 2;
        if col_idx < self.column_widths.len() {
            self.column_widths[col_idx] = self.column_widths[col_idx].max(needed);
        }
        self.current_row.push(cell);
    }

    fn finish_row(&mut self) {
        if !self.current_row.is_empty() {
            self.rows.push(std::mem::take(&mut self.current_row));
        }
    }

    fn cell_width(&self, col_idx: usize) -> usize {
        self.column_widths.get(col_idx).copied().unwrap_or(0).max(3)
    }

    fn render(&self, theme: &Theme) -> Vec<Line> {
        if self.rows.is_empty() {
            return Vec::new();
        }

        let num_cols = self.column_widths.len();
        if num_cols == 0 {
            return Vec::new();
        }

        let mut lines = Vec::new();
        let border_style = Style::fg(theme.muted());
        lines.push(self.render_border(num_cols, '┌', '┬', '┐', border_style));

        for (row_idx, row) in self.rows.iter().enumerate() {
            let max_cell_lines = (0..num_cols)
                .map(|col_idx| row.get(col_idx).map_or(1, |cell| cell.lines.len().max(1)))
                .max()
                .unwrap_or(1);

            for line_idx in 0..max_cell_lines {
                let mut line = Line::default();
                line.push_span(Span::with_style("│", border_style));

                for col_idx in 0..num_cols {
                    let width = self.cell_width(col_idx);
                    let cell = row.get(col_idx);
                    let alignment = cell.map_or_else(|| self.alignments[col_idx], |c| c.alignment);
                    let content_line = cell.and_then(|c| c.lines.get(line_idx));
                    Self::push_formatted_cell_line(&mut line, content_line, width, alignment);

                    if col_idx < num_cols - 1 {
                        line.push_span(Span::with_style("│", border_style));
                    }
                }

                line.push_span(Span::with_style("│", border_style));
                lines.push(line);
            }

            if row_idx == 0 {
                lines.push(self.render_border(num_cols, '├', '┼', '┤', border_style));
            }
        }

        lines.push(self.render_border(num_cols, '└', '┴', '┘', border_style));
        lines
    }

    fn render_border(
        &self,
        num_cols: usize,
        left_char: char,
        mid_char: char,
        right_char: char,
        style: Style,
    ) -> Line {
        let mut s = String::new();
        s.push(left_char);
        for col_idx in 0..num_cols {
            for _ in 0..self.cell_width(col_idx) {
                s.push('─');
            }
            if col_idx < num_cols - 1 {
                s.push(mid_char);
            }
        }
        s.push(right_char);
        Line::with_style(s, style)
    }

    fn push_formatted_cell_line(
        line: &mut Line,
        content_line: Option<&Line>,
        width: usize,
        alignment: Alignment,
    ) {
        let cell_width = width.max(3);
        let content_width = content_line.map_or(0, line_display_width);
        let padding = cell_width.saturating_sub(content_width);

        let (left_pad, right_pad) = match alignment {
            Alignment::Right => (padding.saturating_sub(1), 1),
            Alignment::Center => {
                let left = padding / 2;
                let right = padding.saturating_sub(left);
                (left, right)
            }
            _ => (1, padding.saturating_sub(1)),
        };

        if left_pad > 0 {
            line.push_span(Span::with_style(" ".repeat(left_pad), Style::default()));
        }
        if let Some(content) = content_line {
            line.append_line(content);
        }
        if right_pad > 0 {
            line.push_span(Span::with_style(" ".repeat(right_pad), Style::default()));
        }
    }
}

fn line_display_width(line: &Line) -> usize {
    line.spans()
        .iter()
        .map(|span| UnicodeWidthStr::width(span.text()))
        .sum()
}

/// Caches syntax-highlighted output for code blocks by (lang, content).
///
/// Survives across re-renders so completed code blocks aren't re-highlighted
/// on every streaming token.
#[derive(Default)]
pub struct HighlightCache {
    entries: HashMap<String, HashMap<String, Vec<Line>>>,
}

impl HighlightCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get(&self, lang: &str, code: &str) -> Option<&[Line]> {
        self.entries.get(lang)?.get(code).map(Vec::as_slice)
    }

    fn insert(&mut self, lang: String, code: String, lines: Vec<Line>) {
        self.entries.entry(lang).or_default().insert(code, lines);
    }
}

pub fn render_markdown(text: &str, theme: &Theme, cache: &mut HighlightCache) -> Vec<Line> {
    let renderer = MarkdownRenderer::new(theme, cache);
    renderer.render(text)
}

struct MarkdownRenderer<'a> {
    theme: &'a Theme,
    highlight_cache: &'a mut HighlightCache,
    lines: Vec<Line>,
    current_line: Line,
    style_stack: Vec<Style>,
    /// Stack of list counters: None = unordered, Some(n) = ordered at n
    list_stack: Vec<Option<u64>>,
    /// Accumulated code block text
    code_buffer: String,
    /// Language hint for the current code block
    code_lang: String,
    /// Whether we're inside a code block (accumulating text)
    in_code_block: bool,
    /// Current blockquote nesting depth
    blockquote_depth: usize,
    /// Table state when rendering tables
    table_state: Option<TableState>,
    /// Active cell when parsing inline table content.
    active_cell: Option<CellBuilder>,
}

impl<'a> MarkdownRenderer<'a> {
    fn new(theme: &'a Theme, highlight_cache: &'a mut HighlightCache) -> Self {
        Self {
            theme,
            highlight_cache,
            lines: Vec::new(),
            current_line: Line::default(),
            style_stack: Vec::new(),
            list_stack: Vec::new(),
            code_buffer: String::new(),
            code_lang: String::new(),
            in_code_block: false,
            blockquote_depth: 0,
            table_state: None,
            active_cell: None,
        }
    }

    fn render(mut self, text: &str) -> Vec<Line> {
        let options = Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TABLES;
        let parser = Parser::new_ext(text, options);

        for event in parser {
            self.handle_event(event);
        }

        self.flush_line();

        // Remove trailing empty lines
        while self.lines.last().is_some_and(Line::is_empty) {
            self.lines.pop();
        }

        self.lines
    }

    fn handle_event(&mut self, event: Event<'_>) {
        match event {
            Event::Start(tag) => self.handle_start(tag),
            Event::End(tag_end) => self.handle_end(tag_end),
            Event::Text(text) => self.push_inline_text(&text),
            Event::Code(code) => self.push_inline_code(&code),
            Event::SoftBreak => self.push_soft_break(),
            Event::HardBreak => self.push_hard_break(),
            Event::Rule => {
                self.finish_current_line();
                self.lines.push(Line::with_style(
                    "───────────────",
                    Style::fg(self.theme.muted()),
                ));
                self.lines.push(Line::default());
            }
            _ => {}
        }
    }

    fn handle_start(&mut self, tag: Tag<'_>) {
        match tag {
            Tag::Heading { level, .. } => {
                self.finish_current_line();
                let prefix = "#".repeat(level as usize);
                self.push_styled_text(
                    &format!("{prefix} "),
                    Style::fg(self.theme.heading()).bold(),
                );
                self.style_stack
                    .push(Style::fg(self.theme.heading()).bold());
            }
            Tag::Strong => {
                self.style_stack.push(Style::default().bold());
            }
            Tag::Emphasis => {
                self.style_stack.push(Style::default().italic());
            }
            Tag::Strikethrough => {
                self.style_stack.push(Style::default().strikethrough());
            }
            Tag::BlockQuote(_) => {
                self.finish_current_line();
                self.blockquote_depth += 1;
                self.style_stack.push(Style::fg(self.theme.blockquote()));
            }
            Tag::CodeBlock(kind) => {
                self.finish_current_line();
                self.in_code_block = true;
                self.code_buffer.clear();
                self.code_lang = match kind {
                    CodeBlockKind::Fenced(lang) => {
                        lang.split(',').next().unwrap_or("").trim().to_string()
                    }
                    CodeBlockKind::Indented => String::new(),
                };
            }
            Tag::List(start) => {
                if self.list_stack.is_empty() {
                    self.finish_current_line();
                }
                self.list_stack.push(start);
            }
            Tag::Item => {
                self.flush_line();
                let indent = "  ".repeat(self.list_stack.len().saturating_sub(1));
                let marker = match self.list_stack.last_mut() {
                    Some(Some(n)) => {
                        let marker = format!("{n}. ");
                        *n += 1;
                        marker
                    }
                    _ => "- ".to_string(),
                };
                self.push_styled_text(&format!("{indent}{marker}"), Style::fg(self.theme.muted()));
            }
            Tag::Link { dest_url, .. } => {
                self.style_stack
                    .push(Style::fg(self.theme.link()).underline());
                // Store URL to emit after text if desired; for now just style the text
                let _ = dest_url;
            }
            // Table event handlers
            Tag::Table(alignments) => {
                self.finish_current_line();
                self.table_state = Some(TableState::new(&alignments));
            }
            Tag::TableRow => {
                // Start a new row
                if let Some(ref mut table) = self.table_state {
                    table.start_row();
                }
            }
            Tag::TableCell => {
                self.active_cell = Some(CellBuilder::default());
            }
            _ => {}
        }
    }

    fn handle_end(&mut self, tag_end: TagEnd) {
        match tag_end {
            TagEnd::Paragraph => {
                self.flush_line();
                self.lines.push(Line::default());
            }
            TagEnd::Heading(_) => {
                self.style_stack.pop();
                self.flush_line();
                self.lines.push(Line::default());
            }
            TagEnd::Strong | TagEnd::Emphasis | TagEnd::Strikethrough | TagEnd::Link => {
                self.style_stack.pop();
            }
            TagEnd::BlockQuote(_) => {
                self.style_stack.pop();
                self.blockquote_depth -= 1;
                self.flush_line();
                if self.blockquote_depth == 0 {
                    self.lines.push(Line::default());
                }
            }
            TagEnd::CodeBlock => {
                self.in_code_block = false;
                let code = std::mem::take(&mut self.code_buffer);
                let lang = std::mem::take(&mut self.code_lang);
                if let Some(cached) = self.highlight_cache.get(&lang, &code) {
                    self.lines.extend_from_slice(cached);
                } else {
                    let code_lines = highlight_code(&code, &lang, self.theme);
                    self.highlight_cache.insert(lang, code, code_lines.clone());
                    self.lines.extend(code_lines);
                }
                self.lines.push(Line::default());
            }
            TagEnd::List(_) => {
                self.list_stack.pop();
                if self.list_stack.is_empty() {
                    self.flush_line();
                    self.lines.push(Line::default());
                }
            }
            TagEnd::Item => {
                self.flush_line();
            }
            // Table end event handlers
            TagEnd::Table => {
                if let Some(table) = self.table_state.take() {
                    let rendered = table.render(self.theme);
                    self.lines.extend(rendered);
                    self.lines.push(Line::default());
                }
            }
            TagEnd::TableRow | TagEnd::TableHead => {
                if let Some(ref mut table) = self.table_state {
                    table.finish_row();
                }
            }
            TagEnd::TableCell => {
                if let Some(builder) = self.active_cell.take()
                    && let Some(ref mut table) = self.table_state
                {
                    let col_idx = table.current_row.len();
                    let alignment = table
                        .alignments
                        .get(col_idx)
                        .copied()
                        .unwrap_or(Alignment::None);
                    let lines = builder.finish();
                    let max_width = lines.iter().map(line_display_width).max().unwrap_or(0);
                    let cell = TableCell {
                        lines,
                        alignment,
                        max_width,
                    };
                    table.add_cell(cell);
                }
            }
            _ => {}
        }
    }

    fn current_style(&self) -> Style {
        self.style_stack
            .iter()
            .copied()
            .fold(Style::default(), Style::merge)
    }

    fn push_text(&mut self, text: &str) {
        let style = self.current_style();
        let prefix = self.blockquote_prefix();

        for (i, chunk) in text.split('\n').enumerate() {
            if i > 0 {
                self.flush_line();
            }
            if self.current_line.is_empty() && !prefix.is_empty() {
                self.current_line
                    .push_with_style(&*prefix, Style::fg(self.theme.blockquote()));
            }
            if !chunk.is_empty() {
                self.current_line.push_span(Span::with_style(chunk, style));
            }
        }
    }

    fn push_styled_text(&mut self, text: &str, style: Style) {
        self.current_line.push_span(Span::with_style(text, style));
    }

    fn push_inline_text(&mut self, text: &str) {
        if self.in_code_block {
            self.code_buffer.push_str(text);
            return;
        }

        let style = self.current_style();
        if let Some(cell) = self.active_cell.as_mut() {
            cell.push_text(text, style);
            return;
        }

        self.push_text(text);
    }

    fn push_inline_code(&mut self, code: &str) {
        if self.in_code_block {
            self.code_buffer.push_str(code);
            return;
        }

        let style = Style::fg(self.theme.code_fg());
        if let Some(cell) = self.active_cell.as_mut() {
            cell.push_code(code, style);
        } else {
            self.current_line.push_span(Span::with_style(code, style));
        }
    }

    fn push_soft_break(&mut self) {
        if self.in_code_block {
            self.code_buffer.push('\n');
            return;
        }

        let style = self.current_style();
        if let Some(cell) = self.active_cell.as_mut() {
            cell.soft_break(style);
            return;
        }

        self.push_text(" ");
    }

    fn push_hard_break(&mut self) {
        if self.in_code_block {
            self.code_buffer.push('\n');
            return;
        }

        if let Some(cell) = self.active_cell.as_mut() {
            cell.hard_break();
            return;
        }

        self.flush_line();
    }

    /// Flush the current line only if it has content. Avoids pushing
    /// empty lines at block-element boundaries.
    fn finish_current_line(&mut self) {
        if !self.current_line.is_empty() {
            self.flush_line();
        }
    }

    fn flush_line(&mut self) {
        let prefix = self.blockquote_prefix();
        if !prefix.is_empty() && self.current_line.is_empty() {
            self.current_line
                .push_with_style(&*prefix, Style::fg(self.theme.blockquote()));
        }
        let line = std::mem::take(&mut self.current_line);
        self.lines.push(line);
    }

    fn blockquote_prefix(&self) -> Cow<'static, str> {
        if self.blockquote_depth == 0 {
            Cow::Borrowed("")
        } else {
            Cow::Owned("  ".repeat(self.blockquote_depth))
        }
    }
}

fn highlight_code(code: &str, lang: &str, theme: &Theme) -> Vec<Line> {
    let syntax = find_syntax_by_token(lang);

    let Some(syntax) = syntax else {
        return plain_code_lines(code, theme);
    };

    let syntect_theme = theme.syntect_theme();
    let mut h = HighlightLines::new(syntax, syntect_theme);
    let mut lines = Vec::new();

    for source_line in code.lines() {
        let Ok(ranges) = h.highlight_line(source_line, syntax_set()) else {
            lines.push(Line::with_style(source_line, Style::fg(theme.code_fg())));
            continue;
        };

        let mut line = Line::default();
        for (syntect_style, text) in ranges {
            line.push_span(Span::with_style(text, syntect_to_wisp_style(syntect_style)));
        }
        lines.push(line);
    }

    lines
}

fn plain_code_lines(code: &str, theme: &Theme) -> Vec<Line> {
    let style = Style::fg(theme.code_fg());
    code.lines()
        .map(|line| Line::with_style(line, style))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use unicode_width::UnicodeWidthStr;

    fn test_theme() -> Theme {
        Theme::default()
    }

    fn render(md: &str) -> Vec<Line> {
        let mut cache = HighlightCache::new();
        render_markdown(md, &test_theme(), &mut cache)
    }

    fn render_with_theme(md: &str, theme: &Theme) -> Vec<Line> {
        let mut cache = HighlightCache::new();
        render_markdown(md, theme, &mut cache)
    }

    #[test]
    fn plain_text_passes_through() {
        let lines = render("hello world");
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].plain_text(), "hello world");
    }

    #[test]
    fn heading_renders_with_prefix_and_style() {
        let lines = render("# Title");
        assert!(!lines.is_empty());
        let text = lines[0].plain_text();
        assert!(text.contains("# Title"));
        // Heading spans should be bold
        assert!(lines[0].spans().iter().any(|s| s.style().bold));
    }

    #[test]
    fn bold_text_is_bold() {
        let lines = render("some **bold** text");
        assert_eq!(lines.len(), 1);
        let spans = lines[0].spans();
        let bold_span = spans.iter().find(|s| s.text().contains("bold")).unwrap();
        assert!(bold_span.style().bold);
    }

    #[test]
    fn italic_text_is_italic() {
        let lines = render("some *italic* text");
        assert_eq!(lines.len(), 1);
        let spans = lines[0].spans();
        let italic_span = spans.iter().find(|s| s.text().contains("italic")).unwrap();
        assert!(italic_span.style().italic);
    }

    #[test]
    fn strikethrough_text() {
        let lines = render("some ~~struck~~ text");
        assert_eq!(lines.len(), 1);
        let spans = lines[0].spans();
        let struck_span = spans.iter().find(|s| s.text().contains("struck")).unwrap();
        assert!(struck_span.style().strikethrough);
    }

    #[test]
    fn inline_code_has_code_style() {
        let theme = test_theme();
        let lines = render_with_theme("use `foo()` here", &theme);
        assert_eq!(lines.len(), 1);
        let spans = lines[0].spans();
        let code_span = spans.iter().find(|s| s.text().contains("foo()")).unwrap();
        assert_eq!(code_span.style().fg, Some(theme.code_fg()));
        assert_eq!(code_span.style().bg, None);
    }

    #[test]
    fn fenced_code_block_produces_lines() {
        let md = "```rust\nfn main() {}\n```";
        let lines = render(md);
        let text: String = lines
            .iter()
            .map(Line::plain_text)
            .collect::<Vec<_>>()
            .join("\n");
        assert!(text.contains("fn main()"));
    }

    #[test]
    fn unordered_list() {
        let md = "- alpha\n- beta\n- gamma";
        let lines = render(md);
        let texts: Vec<String> = lines.iter().map(Line::plain_text).collect();
        assert!(texts.iter().any(|t| t.contains("- alpha")));
        assert!(texts.iter().any(|t| t.contains("- beta")));
        assert!(texts.iter().any(|t| t.contains("- gamma")));
    }

    #[test]
    fn ordered_list() {
        let md = "1. first\n2. second\n3. third";
        let lines = render(md);
        let texts: Vec<String> = lines.iter().map(Line::plain_text).collect();
        assert!(texts.iter().any(|t| t.contains("1. first")));
        assert!(texts.iter().any(|t| t.contains("2. second")));
        assert!(texts.iter().any(|t| t.contains("3. third")));
    }

    #[test]
    fn blockquote_is_indented() {
        let md = "> quoted text";
        let theme = test_theme();
        let lines = render_with_theme(md, &theme);
        let texts: Vec<String> = lines.iter().map(Line::plain_text).collect();
        assert!(texts.iter().any(|t| t.contains("quoted text")));
        let quoted_line = lines
            .iter()
            .find(|l| l.plain_text().contains("quoted"))
            .unwrap();
        assert!(quoted_line.plain_text().starts_with("  quoted text"));
        assert!(
            quoted_line
                .spans()
                .iter()
                .any(|s| s.style().fg == Some(theme.blockquote()))
        );
        assert!(!quoted_line.spans().iter().any(|s| s.style().dim));
    }

    #[test]
    fn horizontal_rule() {
        let md = "above\n\n---\n\nbelow";
        let lines = render(md);
        let texts: Vec<String> = lines.iter().map(Line::plain_text).collect();
        assert!(texts.iter().any(|t| t.contains("───")));
    }

    #[test]
    fn link_is_underlined() {
        let md = "click [here](https://example.com)";
        let theme = test_theme();
        let lines = render_with_theme(md, &theme);
        assert_eq!(lines.len(), 1);
        let spans = lines[0].spans();
        let link_span = spans.iter().find(|s| s.text().contains("here")).unwrap();
        assert!(link_span.style().underline);
        assert_eq!(link_span.style().fg, Some(theme.link()));
    }

    #[test]
    fn empty_input_returns_empty() {
        let lines = render("");
        assert!(lines.is_empty());
    }

    #[test]
    fn multiple_paragraphs_have_spacing() {
        let md = "para one\n\npara two";
        let lines = render(md);
        // Should be: "para one", empty, "para two" (trailing empty stripped)
        assert!(lines.len() >= 3);
        assert!(lines.iter().any(Line::is_empty));
    }

    #[test]
    fn nested_bold_italic() {
        let md = "***bold and italic***";
        let lines = render(md);
        assert_eq!(lines.len(), 1);
        let spans = lines[0].spans();
        let span = spans
            .iter()
            .find(|s| s.text().contains("bold and italic"))
            .unwrap();
        assert!(span.style().bold);
        assert!(span.style().italic);
    }

    #[test]
    fn unknown_language_falls_back_to_plain() {
        let md = "```nosuchlang\nsome code\n```";
        let theme = test_theme();
        let lines = render_with_theme(md, &theme);
        let text: String = lines
            .iter()
            .map(Line::plain_text)
            .collect::<Vec<_>>()
            .join("\n");
        assert!(text.contains("some code"));
        // Should have code styling even without highlighting
        let code_line = lines
            .iter()
            .find(|l| l.plain_text().contains("some code"))
            .unwrap();
        assert_eq!(code_line.spans()[0].style().fg, Some(theme.code_fg()));
    }

    #[test]
    fn nested_list_indents() {
        let md = "- outer\n  - inner";
        let lines = render(md);
        let texts: Vec<String> = lines.iter().map(Line::plain_text).collect();
        assert!(texts.iter().any(|t| t.contains("outer")));
        assert!(texts.iter().any(|t| t.contains("inner")));
        // Inner item should have more leading whitespace
        let inner = texts.iter().find(|t| t.contains("inner")).unwrap();
        let outer = texts.iter().find(|t| t.contains("outer")).unwrap();
        let inner_indent = inner.len() - inner.trim_start().len();
        let outer_indent = outer.len() - outer.trim_start().len();
        assert!(inner_indent > outer_indent);
    }

    #[test]
    fn highlight_cache_returns_cached_code_block() {
        let mut cache = HighlightCache::new();
        let md = "```rust\nfn main() {}\n```";
        let first = render_markdown(md, &test_theme(), &mut cache);
        let second = render_markdown(md, &test_theme(), &mut cache);
        assert_eq!(first, second);
        // Cache should have an entry now
        assert!(cache.get("rust", "fn main() {}\n").is_some());
    }

    #[test]
    fn highlight_cache_not_affected_by_different_code_block() {
        let mut cache = HighlightCache::new();
        let md1 = "```rust\nfn a() {}\n```";
        let md2 = "```rust\nfn b() {}\n```";
        let lines1 = render_markdown(md1, &test_theme(), &mut cache);
        let lines2 = render_markdown(md2, &test_theme(), &mut cache);
        // Both should be cached independently
        assert!(cache.get("rust", "fn a() {}\n").is_some());
        assert!(cache.get("rust", "fn b() {}\n").is_some());
        // And produce different output
        assert_ne!(
            lines1.iter().map(Line::plain_text).collect::<String>(),
            lines2.iter().map(Line::plain_text).collect::<String>(),
        );
    }

    #[test]
    fn simple_table_renders_correctly() {
        let md =
            "| Name | Age | City |\n|------|-----|------|\n| Alice | 30 | NYC |\n| Bob | 25 | LA |";
        let lines = render(md);
        let texts: Vec<String> = lines.iter().map(Line::plain_text).collect();
        let all_text = texts.join("\n");
        let non_empty_lines: Vec<&String> = texts.iter().filter(|t| !t.is_empty()).collect();

        // Verify table structure with unicode borders
        assert!(
            all_text.contains('┌'),
            "Should have top-left corner: {all_text}",
        );
        assert!(all_text.contains('┐'), "Should have top-right corner");
        assert!(all_text.contains('┬'), "Should have top T-junction");
        assert!(all_text.contains('┼'), "Should have cross junction");
        assert!(all_text.contains('┴'), "Should have bottom T-junction");
        assert!(all_text.contains('├'), "Should have left T-junction");
        assert!(all_text.contains('┤'), "Should have right T-junction");
        assert!(all_text.contains('└'), "Should have bottom-left corner");
        assert!(all_text.contains('┘'), "Should have bottom-right corner");
        assert!(all_text.contains('│'), "Should have vertical border");
        assert_eq!(texts.iter().filter(|t| t.contains('┼')).count(), 1);
        assert_eq!(non_empty_lines.len(), 6);

        // Verify content
        assert!(all_text.contains("Alice"));
        assert!(all_text.contains("30"));
        assert!(all_text.contains("NYC"));
        assert!(all_text.contains("Bob"));
        assert!(all_text.contains("25"));
        assert!(all_text.contains("LA"));
        assert!(
            !texts.iter().any(|t| t.trim() == "Alice"),
            "Table content leaked to standalone line: {texts:?}"
        );
    }

    #[test]
    fn table_with_alignment() {
        let md = "| Left | Center | Right |\n|:-----|:------:|------:|\n| L | C | R |";
        let lines = render(md);
        let texts: Vec<String> = lines.iter().map(Line::plain_text).collect();
        let all_text = texts.join("\n");

        // Verify alignment markers are present
        assert!(all_text.contains("Left"));
        assert!(all_text.contains("Center"));
        assert!(all_text.contains("Right"));
    }

    #[test]
    fn table_with_empty_cells() {
        let md = "| A | B | C |\n|---|---|---|\n| 1 |   | 3 |";
        let lines = render(md);
        let texts: Vec<String> = lines.iter().map(Line::plain_text).collect();
        let all_text = texts.join("\n");

        // Should render without error
        assert!(all_text.contains('┌'));
        assert!(all_text.contains('1'));
        assert!(all_text.contains('3'));
    }

    #[test]
    fn table_cell_inline_code_does_not_leak_line() {
        let md = "| A | B |\n|---|---|\n| `x` and **y** | z |";
        let lines = render(md);
        let texts: Vec<String> = lines.iter().map(Line::plain_text).collect();
        let non_empty_lines: Vec<&String> = texts.iter().filter(|t| !t.is_empty()).collect();

        assert_eq!(non_empty_lines.len(), 5);
        assert!(
            !texts.iter().any(|t| t.trim() == "x"),
            "Inline code leaked outside table: {texts:?}"
        );
        let body_row = texts
            .iter()
            .find(|t| t.starts_with('│') && t.contains("and y"))
            .expect("Expected a rendered table body row");
        assert!(body_row.contains('x'));
    }

    #[test]
    fn table_cell_preserves_inline_styles() {
        let theme = test_theme();
        let md =
            "| A | B | C |\n|---|---|---|\n| **bold** | [link](https://example.com) | `code` |";
        let lines = render_with_theme(md, &theme);

        let bold_span = lines
            .iter()
            .flat_map(|line| line.spans().iter())
            .find(|span| span.text() == "bold")
            .expect("Expected bold span inside table");
        assert!(bold_span.style().bold);

        let link_span = lines
            .iter()
            .flat_map(|line| line.spans().iter())
            .find(|span| span.text() == "link")
            .expect("Expected link span inside table");
        assert!(link_span.style().underline);
        assert_eq!(link_span.style().fg, Some(theme.link()));

        let code_span = lines
            .iter()
            .flat_map(|line| line.spans().iter())
            .find(|span| span.text() == "code")
            .expect("Expected inline code span inside table");
        assert_eq!(code_span.style().fg, Some(theme.code_fg()));
    }

    fn row_inner_display_widths(row: &str) -> Vec<usize> {
        let segments: Vec<&str> = row.split('│').collect();
        if segments.len() < 3 {
            return Vec::new();
        }
        segments[1..segments.len() - 1]
            .iter()
            .map(|segment| UnicodeWidthStr::width(*segment))
            .collect()
    }

    #[test]
    fn table_unicode_alignment_uses_display_width() {
        let md = "| Left | Right |\n|------|-------|\n| a | 你 |\n| bb | 😀 |";
        let lines = render(md);
        let row_texts: Vec<String> = lines
            .iter()
            .map(Line::plain_text)
            .filter(|text| text.starts_with('│'))
            .collect();

        assert!(row_texts.len() >= 3);
        let expected_widths = row_inner_display_widths(&row_texts[0]);
        for row in row_texts.iter().skip(1) {
            assert_eq!(row_inner_display_widths(row), expected_widths);
        }
    }

    #[test]
    fn table_row_cell_count_normalization() {
        let md = "| A | B | C |\n|---|---|---|\n| 1 | 2 |\n| 3 | 4 | 5 | 6 |";
        let lines = render(md);
        let row_texts: Vec<String> = lines
            .iter()
            .map(Line::plain_text)
            .filter(|text| text.starts_with('│'))
            .collect();

        assert_eq!(row_texts.len(), 3);
        for row in &row_texts {
            assert_eq!(row.matches('│').count(), 4);
            assert_eq!(row_inner_display_widths(row).len(), 3);
        }
    }

    #[test]
    fn table_in_paragraph_context() {
        let md = "Here is a table:\n\n| Item | Price |\n|------|-------|\n| Apple | $1.00 |\n| Orange | $1.50 |\n\nThat's the table.";
        let lines = render(md);
        let texts: Vec<String> = lines.iter().map(Line::plain_text).collect();
        let all_text = texts.join("\n");

        // Verify surrounding text is preserved
        assert!(all_text.contains("Here is a table:"));
        assert!(all_text.contains("That's the table."));
        // Verify table
        assert!(all_text.contains("Apple"));
        assert!(all_text.contains("$1.00"));
    }

    #[test]
    fn table_single_column() {
        let md = "| Value |\n|--------|\n| Hello |";
        let lines = render(md);
        let texts: Vec<String> = lines.iter().map(Line::plain_text).collect();
        let all_text = texts.join("\n");

        assert!(all_text.contains('┌'));
        assert!(all_text.contains('┐'));
        assert!(all_text.contains("Hello"));
    }
}
