use std::sync::LazyLock;

use crossterm::style::Color;
use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag, TagEnd};
use syntect::easy::HighlightLines;
use syntect::highlighting::{
    FontStyle, ScopeSelectors, StyleModifier, ThemeItem, ThemeSettings,
};
use syntect::parsing::SyntaxSet;

use super::screen::{Line, Span, Style};
use super::theme::Theme;

pub fn render_markdown(text: &str, theme: &Theme) -> Vec<Line> {
    let renderer = MarkdownRenderer::new(theme);
    renderer.render(text)
}

struct SyntectState {
    syntax_set: SyntaxSet,
    theme: syntect::highlighting::Theme,
}

static SYNTECT: LazyLock<SyntectState> = LazyLock::new(|| SyntectState {
    syntax_set: SyntaxSet::load_defaults_newlines(),
    theme: build_ayu_dark_theme(),
});

struct MarkdownRenderer<'a> {
    theme: &'a Theme,
    lines: Vec<Line>,
    current_line: Line,
    style_stack: Vec<Style>,
    /// Nesting depth for list indentation
    list_depth: usize,
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
}

impl<'a> MarkdownRenderer<'a> {
    fn new(theme: &'a Theme) -> Self {
        Self {
            theme,
            lines: Vec::new(),
            current_line: Line::default(),
            style_stack: Vec::new(),
            list_depth: 0,
            list_stack: Vec::new(),
            code_buffer: String::new(),
            code_lang: String::new(),
            in_code_block: false,
            blockquote_depth: 0,
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
            Event::Text(text) => {
                if self.in_code_block {
                    self.code_buffer.push_str(&text);
                } else {
                    self.push_text(&text);
                }
            }
            Event::Code(code) => self.push_inline_code(&code),
            Event::SoftBreak => {
                if self.in_code_block {
                    self.code_buffer.push('\n');
                } else {
                    self.push_text(" ");
                }
            }
            Event::HardBreak => {
                if self.in_code_block {
                    self.code_buffer.push('\n');
                } else {
                    self.flush_line();
                }
            }
            Event::Rule => {
                self.finish_current_line();
                self.lines
                    .push(Line::with_style("───────────────", Style::fg(self.theme.muted)));
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
                self.push_styled_text(&format!("{prefix} "), Style::fg(self.theme.heading).bold());
                self.style_stack
                    .push(Style::fg(self.theme.heading).bold());
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
                self.style_stack.push(Style::fg(self.theme.blockquote).dim());
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
                if self.list_depth == 0 {
                    self.finish_current_line();
                }
                self.list_stack.push(start);
                self.list_depth += 1;
            }
            Tag::Item => {
                self.flush_line();
                let indent = "  ".repeat(self.list_depth.saturating_sub(1));
                let marker = match self.list_stack.last_mut() {
                    Some(Some(n)) => {
                        let marker = format!("{n}. ");
                        *n += 1;
                        marker
                    }
                    _ => "- ".to_string(),
                };
                self.push_styled_text(&format!("{indent}{marker}"), Style::fg(self.theme.muted));
            }
            Tag::Link { dest_url, .. } => {
                self.style_stack
                    .push(Style::fg(self.theme.link).underline());
                // Store URL to emit after text if desired; for now just style the text
                let _ = dest_url;
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
                let code_lines = highlight_code(&code, &lang, self.theme);
                self.lines.extend(code_lines);
                self.lines.push(Line::default());
            }
            TagEnd::List(_) => {
                self.list_stack.pop();
                self.list_depth -= 1;
                if self.list_depth == 0 {
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
            if i > 0 && !prefix.is_empty() {
                self.current_line
                    .push_with_style(&prefix, Style::fg(self.theme.blockquote));
            }
            if !chunk.is_empty() {
                self.current_line.push_span(Span::with_style(chunk, style));
            }
        }
    }

    fn push_styled_text(&mut self, text: &str, style: Style) {
        self.current_line.push_span(Span::with_style(text, style));
    }

    fn push_inline_code(&mut self, code: &str) {
        let style = Style::fg(self.theme.code_fg);
        self.current_line
            .push_span(Span::with_style(code, style));
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
                .push_with_style(&prefix, Style::fg(self.theme.blockquote));
        }
        let line = std::mem::take(&mut self.current_line);
        self.lines.push(line);
    }

    fn blockquote_prefix(&self) -> String {
        if self.blockquote_depth == 0 {
            String::new()
        } else {
            "  ".repeat(self.blockquote_depth)
        }
    }
}

fn highlight_code(code: &str, lang: &str, theme: &Theme) -> Vec<Line> {
    let st = &*SYNTECT;

    let syntax = if lang.is_empty() {
        None
    } else {
        st.syntax_set.find_syntax_by_token(lang)
    };

    let Some(syntax) = syntax else {
        return plain_code_lines(code, theme);
    };

    let mut h = HighlightLines::new(syntax, &st.theme);
    let mut lines = Vec::new();

    for source_line in code.lines() {
        let Ok(ranges) = h.highlight_line(source_line, &st.syntax_set) else {
            lines.push(Line::with_style(
                source_line,
                Style::fg(theme.code_fg),
            ));
            continue;
        };

        let mut line = Line::default();
        for (syntect_style, text) in ranges {
            line.push_span(Span::with_style(text, syntect_to_wisp_style(syntect_style, theme)));
        }
        lines.push(line);
    }

    lines
}

fn plain_code_lines(code: &str, theme: &Theme) -> Vec<Line> {
    let style = Style::fg(theme.code_fg);
    code.lines()
        .map(|line| Line::with_style(line, style))
        .collect()
}

fn syntect_to_wisp_style(s: syntect::highlighting::Style, theme: &Theme) -> Style {
    let fg = Color::Rgb {
        r: s.foreground.r,
        g: s.foreground.g,
        b: s.foreground.b,
    };

    let mut style = Style::fg(fg);
    if s.font_style.contains(FontStyle::BOLD) {
        style = style.bold();
    }
    if s.font_style.contains(FontStyle::ITALIC) {
        style = style.italic();
    }
    if s.font_style.contains(FontStyle::UNDERLINE) {
        style = style.underline();
    }
    style
}

/// Build the ayu-dark syntax highlighting theme from embedded color values.
///
/// Color palette sourced from <https://github.com/ayu-theme/ayu-colors>.
#[allow(clippy::unreadable_literal)]
fn build_ayu_dark_theme() -> syntect::highlighting::Theme {
    use syntect::highlighting::Color as SC;

    let c = |hex: u32| SC {
        r: ((hex >> 16) & 0xFF) as u8,
        g: ((hex >> 8) & 0xFF) as u8,
        b: (hex & 0xFF) as u8,
        a: 0xFF,
    };

    let rule = |scope: &str, fg: u32| ThemeItem {
        scope: scope.parse::<ScopeSelectors>().unwrap(),
        style: StyleModifier {
            foreground: Some(c(fg)),
            background: None,
            font_style: None,
        },
    };

    let rule_italic = |scope: &str, fg: u32| ThemeItem {
        scope: scope.parse::<ScopeSelectors>().unwrap(),
        style: StyleModifier {
            foreground: Some(c(fg)),
            background: None,
            font_style: Some(FontStyle::ITALIC),
        },
    };

    syntect::highlighting::Theme {
        name: Some("ayu-dark".to_string()),
        author: Some("ayu-theme".to_string()),
        settings: ThemeSettings {
            foreground: Some(c(0xBFBDB6)),
            background: Some(c(0x10141C)),
            caret: Some(c(0xE6B450)),
            selection: Some(c(0x3388FF)),
            ..ThemeSettings::default()
        },
        scopes: vec![
            rule_italic("comment", 0xACB6BF),
            rule("string, constant.other.symbol, string.quoted", 0xAAD94C),
            rule("string.regexp, constant.character, constant.other", 0x95E6CB),
            rule("constant.numeric", 0xE6B450),
            rule("constant.language", 0xE6B450),
            rule("meta.constant, entity.name.constant", 0xD2A6FF),
            rule("variable", 0xBFBDB6),
            rule("variable.member", 0xF07178),
            rule_italic("variable.language", 0x39BAE6),
            rule("storage, storage.type.keyword", 0xFF8F40),
            rule("keyword", 0xFF8F40),
            rule("keyword.operator", 0xF29668),
            rule("punctuation.separator, punctuation.terminator", 0xBFBDB6),
            rule("punctuation.section", 0xBFBDB6),
            rule("punctuation.accessor", 0xF29668),
            rule("entity.other.inherited-class", 0x39BAE6),
            rule("storage.type.function", 0xFF8F40),
            rule("entity.name.function", 0xFFB454),
            rule("variable.parameter, meta.parameter", 0xD2A6FF),
            rule(
                "variable.function, variable.annotation, meta.function-call.generic, support.function.go",
                0xFFB454,
            ),
            rule("support.function, support.macro", 0xF07178),
            rule("entity.name.import, entity.name.package", 0xAAD94C),
            rule("entity.name", 0x59C2FF),
            rule("entity.name.tag, meta.tag.sgml", 0x39BAE6),
            rule("entity.other.attribute-name", 0xFFB454),
            rule_italic("support.constant", 0xF29668),
            rule("support.type, support.class", 0x39BAE6),
            rule("meta.decorator variable.other, meta.decorator punctuation.decorator, storage.type.annotation, variable.annotation, punctuation.definition.annotation", 0xE6B673),
            rule("invalid", 0xD95757),
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_theme() -> Theme {
        Theme::default()
    }

    #[test]
    fn plain_text_passes_through() {
        let lines = render_markdown("hello world", &test_theme());
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].plain_text(), "hello world");
    }

    #[test]
    fn heading_renders_with_prefix_and_style() {
        let lines = render_markdown("# Title", &test_theme());
        assert!(!lines.is_empty());
        let text = lines[0].plain_text();
        assert!(text.contains("# Title"));
        // Heading spans should be bold
        assert!(lines[0].spans().iter().any(|s| s.style().bold));
    }

    #[test]
    fn bold_text_is_bold() {
        let lines = render_markdown("some **bold** text", &test_theme());
        assert_eq!(lines.len(), 1);
        let spans = lines[0].spans();
        let bold_span = spans.iter().find(|s| s.text().contains("bold")).unwrap();
        assert!(bold_span.style().bold);
    }

    #[test]
    fn italic_text_is_italic() {
        let lines = render_markdown("some *italic* text", &test_theme());
        assert_eq!(lines.len(), 1);
        let spans = lines[0].spans();
        let italic_span = spans.iter().find(|s| s.text().contains("italic")).unwrap();
        assert!(italic_span.style().italic);
    }

    #[test]
    fn strikethrough_text() {
        let lines = render_markdown("some ~~struck~~ text", &test_theme());
        assert_eq!(lines.len(), 1);
        let spans = lines[0].spans();
        let struck_span = spans.iter().find(|s| s.text().contains("struck")).unwrap();
        assert!(struck_span.style().strikethrough);
    }

    #[test]
    fn inline_code_has_code_style() {
        let theme = test_theme();
        let lines = render_markdown("use `foo()` here", &theme);
        assert_eq!(lines.len(), 1);
        let spans = lines[0].spans();
        let code_span = spans.iter().find(|s| s.text().contains("foo()")).unwrap();
        assert_eq!(code_span.style().fg, Some(theme.code_fg));
        assert_eq!(code_span.style().bg, None);
    }

    #[test]
    fn fenced_code_block_produces_lines() {
        let md = "```rust\nfn main() {}\n```";
        let lines = render_markdown(md, &test_theme());
        let text: String = lines.iter().map(|l| l.plain_text()).collect::<Vec<_>>().join("\n");
        assert!(text.contains("fn main()"));
    }

    #[test]
    fn unordered_list() {
        let md = "- alpha\n- beta\n- gamma";
        let lines = render_markdown(md, &test_theme());
        let texts: Vec<String> = lines.iter().map(|l| l.plain_text()).collect();
        assert!(texts.iter().any(|t| t.contains("- alpha")));
        assert!(texts.iter().any(|t| t.contains("- beta")));
        assert!(texts.iter().any(|t| t.contains("- gamma")));
    }

    #[test]
    fn ordered_list() {
        let md = "1. first\n2. second\n3. third";
        let lines = render_markdown(md, &test_theme());
        let texts: Vec<String> = lines.iter().map(|l| l.plain_text()).collect();
        assert!(texts.iter().any(|t| t.contains("1. first")));
        assert!(texts.iter().any(|t| t.contains("2. second")));
        assert!(texts.iter().any(|t| t.contains("3. third")));
    }

    #[test]
    fn blockquote_is_indented() {
        let md = "> quoted text";
        let theme = test_theme();
        let lines = render_markdown(md, &theme);
        let texts: Vec<String> = lines.iter().map(|l| l.plain_text()).collect();
        assert!(texts.iter().any(|t| t.contains("quoted text")));
        // Should have some indentation from blockquote prefix
        let quoted_line = lines.iter().find(|l| l.plain_text().contains("quoted")).unwrap();
        assert!(quoted_line.spans().iter().any(|s| s.style().dim));
    }

    #[test]
    fn horizontal_rule() {
        let md = "above\n\n---\n\nbelow";
        let lines = render_markdown(md, &test_theme());
        let texts: Vec<String> = lines.iter().map(|l| l.plain_text()).collect();
        assert!(texts.iter().any(|t| t.contains("───")));
    }

    #[test]
    fn link_is_underlined() {
        let md = "click [here](https://example.com)";
        let theme = test_theme();
        let lines = render_markdown(md, &theme);
        assert_eq!(lines.len(), 1);
        let spans = lines[0].spans();
        let link_span = spans.iter().find(|s| s.text().contains("here")).unwrap();
        assert!(link_span.style().underline);
        assert_eq!(link_span.style().fg, Some(theme.link));
    }

    #[test]
    fn empty_input_returns_empty() {
        let lines = render_markdown("", &test_theme());
        assert!(lines.is_empty());
    }

    #[test]
    fn multiple_paragraphs_have_spacing() {
        let md = "para one\n\npara two";
        let lines = render_markdown(md, &test_theme());
        // Should be: "para one", empty, "para two" (trailing empty stripped)
        assert!(lines.len() >= 3);
        assert!(lines.iter().any(|l| l.is_empty()));
    }

    #[test]
    fn nested_bold_italic() {
        let md = "***bold and italic***";
        let lines = render_markdown(md, &test_theme());
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
        let lines = render_markdown(md, &theme);
        let text: String = lines.iter().map(|l| l.plain_text()).collect::<Vec<_>>().join("\n");
        assert!(text.contains("some code"));
        // Should have code styling even without highlighting
        let code_line = lines.iter().find(|l| l.plain_text().contains("some code")).unwrap();
        assert_eq!(code_line.spans()[0].style().fg, Some(theme.code_fg));
    }

    #[test]
    fn nested_list_indents() {
        let md = "- outer\n  - inner";
        let lines = render_markdown(md, &test_theme());
        let texts: Vec<String> = lines.iter().map(|l| l.plain_text()).collect();
        assert!(texts.iter().any(|t| t.contains("outer")));
        assert!(texts.iter().any(|t| t.contains("inner")));
        // Inner item should have more leading whitespace
        let inner = texts.iter().find(|t| t.contains("inner")).unwrap();
        let outer = texts.iter().find(|t| t.contains("outer")).unwrap();
        let inner_indent = inner.len() - inner.trim_start().len();
        let outer_indent = outer.len() - outer.trim_start().len();
        assert!(inner_indent > outer_indent);
    }
}
