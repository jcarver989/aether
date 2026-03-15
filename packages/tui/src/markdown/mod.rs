mod table;

use pulldown_cmark::{Alignment, CodeBlockKind, Event, Options, Parser, Tag, TagEnd};
use std::borrow::Cow;

use crate::line::Line;
use crate::rendering::render_context::ViewContext;
use crate::span::Span;
use crate::style::Style;
use crate::theme::Theme;

use table::{CellBuilder, TableCell, TableState, line_display_width};

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
            | Tag::Paragraph => self.handle_block_start(&tag),

            Tag::Strong | Tag::Emphasis | Tag::Strikethrough | Tag::Link { .. } => {
                self.handle_inline_start(tag);
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
                self.handle_inline_end(tag_end);
            }

            TagEnd::CodeBlock => self.handle_code_block_end(),

            TagEnd::Table | TagEnd::TableRow | TagEnd::TableHead | TagEnd::TableCell => {
                self.handle_table_end(tag_end);
            }

            _ => {}
        }
    }

    fn handle_block_start(&mut self, tag: &Tag<'_>) {
        match tag {
            Tag::Heading { level, .. } => {
                self.finish_current_line();
                let prefix = "#".repeat(*level as usize);
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
                self.list_stack.push(*start);
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
