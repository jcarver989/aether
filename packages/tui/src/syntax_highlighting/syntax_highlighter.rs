use super::syntect_bridge::{find_syntax_for_hint, syntax_set, syntect_to_wisp_style};
use crate::line::Line;
use crate::span::Span;
use crate::style::Style;
use crate::theme::Theme;
use std::collections::HashMap;
use syntect::easy::HighlightLines;

/// Unified syntax-highlighting facade.
///
/// Results are cached by `(lang, code)` so repeated re-renders
/// (e.g. during streaming) skip the expensive syntect pass.
#[derive(Default)]
pub struct SyntaxHighlighter {
    cache: HashMap<String, HashMap<String, Vec<Line>>>,
}

impl SyntaxHighlighter {
    pub fn new() -> Self {
        Self::default()
    }

    /// Syntax-highlights `code`, caching the result by `(lang, code)`.
    pub fn highlight(&mut self, code: &str, lang: &str, theme: &Theme) -> Vec<Line> {
        if let Some(cached) = self.cache.get(lang).and_then(|m| m.get(code)) {
            return cached.clone();
        }
        let lines = render_highlighted_lines(code, lang, theme);
        self.cache
            .entry(lang.to_string())
            .or_default()
            .insert(code.to_string(), lines.clone());
        lines
    }

    pub fn clear(&mut self) {
        self.cache.clear();
    }
}

fn render_highlighted_lines(code: &str, lang: &str, theme: &Theme) -> Vec<Line> {
    let syntax = find_syntax_for_hint(lang);

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
