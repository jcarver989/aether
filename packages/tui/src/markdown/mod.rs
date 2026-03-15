mod table;

use pulldown_cmark::{Alignment, CodeBlockKind, Event, Options, Parser, Tag, TagEnd};
use std::borrow::Cow;

use crate::line::Line;
use crate::rendering::render_context::ViewContext;
use crate::span::Span;
use crate::style::Style;
use crate::theme::Theme;

use table::{line_display_width, CellBuilder, TableCell, TableState};

pub fn render_markdown(text: &str, context: &ViewContext) -> Vec<Line> {
    let renderer = MarkdownRenderer::new(context);
    renderer.render(text)
}

struct MarkdownRenderer<'a> {
    context: &'a ViewContext,
    theme: &'a Theme,
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
    fn new(context: &'a ViewContext) -> Self {
        Self {
            context,
            theme: &context.theme,
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
            Tag::Heading { .. }
            | Tag::BlockQuote(_)
            | Tag::List(_)
            | Tag::Item
            | Tag::Paragraph => self.handle_block_start(tag),

            Tag::Strong | Tag::Emphasis | Tag::Strikethrough | Tag::Link { .. } => {
                self.handle_inline_start(tag)
            }

            Tag::CodeBlock(_) => self.handle_code_block_start(tag),

            Tag::Table(_) | Tag::TableRow | Tag::TableCell => self.handle_table_start(tag),

            _ => {}
        }
    }

    fn handle_end(&mut self, tag_end: TagEnd) {
        match tag_end {
            TagEnd::Paragraph
            | TagEnd::Heading(_)
            | TagEnd::BlockQuote(_)
            | TagEnd::List(_)
            | TagEnd::Item => self.handle_block_end(tag_end),

            TagEnd::Strong | TagEnd::Emphasis | TagEnd::Strikethrough | TagEnd::Link => {
                self.handle_inline_end(tag_end)
            }

            TagEnd::CodeBlock => self.handle_code_block_end(),

            TagEnd::Table | TagEnd::TableRow | TagEnd::TableHead | TagEnd::TableCell => {
                self.handle_table_end(tag_end)
            }

            _ => {}
        }
    }

    fn handle_block_start(&mut self, tag: Tag<'_>) {
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
            Tag::BlockQuote(_) => {
                self.finish_current_line();
                self.blockquote_depth += 1;
                self.style_stack.push(Style::fg(self.theme.blockquote()));
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
            Tag::Paragraph => {}
            _ => {}
        }
    }

    fn handle_block_end(&mut self, tag_end: TagEnd) {
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
            TagEnd::BlockQuote(_) => {
                self.style_stack.pop();
                self.blockquote_depth -= 1;
                self.flush_line();
                if self.blockquote_depth == 0 {
                    self.lines.push(Line::default());
                }
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
            _ => {}
        }
    }

    fn handle_inline_start(&mut self, tag: Tag<'_>) {
        match tag {
            Tag::Strong => {
                self.style_stack.push(Style::default().bold());
            }
            Tag::Emphasis => {
                self.style_stack.push(Style::default().italic());
            }
            Tag::Strikethrough => {
                self.style_stack.push(Style::default().strikethrough());
            }
            Tag::Link { dest_url, .. } => {
                self.style_stack
                    .push(Style::fg(self.theme.link()).underline());
                // Store URL to emit after text if desired; for now just style the text
                let _ = dest_url;
            }
            _ => {}
        }
    }

    fn handle_inline_end(&mut self, _tag_end: TagEnd) {
        self.style_stack.pop();
    }

    fn handle_code_block_start(&mut self, tag: Tag<'_>) {
        if let Tag::CodeBlock(kind) = tag {
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
    }

    fn handle_code_block_end(&mut self) {
        self.in_code_block = false;
        let code = std::mem::take(&mut self.code_buffer);
        let lang = std::mem::take(&mut self.code_lang);
        let code_lines = self
            .context
            .highlighter()
            .highlight(&code, &lang, self.theme);
        self.lines.extend(code_lines);
        self.lines.push(Line::default());
    }

    fn handle_table_start(&mut self, tag: Tag<'_>) {
        match tag {
            Tag::Table(alignments) => {
                self.finish_current_line();
                self.table_state = Some(TableState::new(&alignments));
            }
            Tag::TableRow => {
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

    fn handle_table_end(&mut self, tag_end: TagEnd) {
        match tag_end {
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

#[cfg(test)]
mod tests {
    use super::*;
    use unicode_width::UnicodeWidthStr;

    fn test_theme() -> Theme {
        Theme::default()
    }

    fn test_context() -> ViewContext {
        ViewContext::new((80, 24))
    }

    fn test_context_with_theme(theme: Theme) -> ViewContext {
        ViewContext::new_with_theme((80, 24), theme)
    }

    fn render(md: &str) -> Vec<Line> {
        render_markdown(md, &test_context())
    }

    fn render_with_theme(md: &str, theme: &Theme) -> Vec<Line> {
        render_markdown(md, &test_context_with_theme(theme.clone()))
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
        let ctx = test_context();
        let md = "```rust\nfn main() {}\n```";
        let first = render_markdown(md, &ctx);
        let second = render_markdown(md, &ctx);
        assert_eq!(first, second);
    }

    #[test]
    fn highlight_cache_not_affected_by_different_code_block() {
        let ctx = test_context();
        let md1 = "```rust\nfn a() {}\n```";
        let md2 = "```rust\nfn b() {}\n```";
        let lines1 = render_markdown(md1, &ctx);
        let lines2 = render_markdown(md2, &ctx);
        // Produce different output
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
