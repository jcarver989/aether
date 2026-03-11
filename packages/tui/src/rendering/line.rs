use crossterm::style::{Attribute, Color, SetAttribute, SetBackgroundColor, SetForegroundColor};
use std::fmt::Write as _;
use unicode_width::UnicodeWidthStr;

use super::soft_wrap;
use super::span::Span;
use super::style::Style;

/// A single line of styled terminal output, composed of [`Span`]s.
///
/// ANSI escape codes are emitted only when [`to_ansi_string`](Line::to_ansi_string)
/// is called, keeping the data model free of formatting concerns.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Line {
    spans: Vec<Span>,
}

impl Line {
    pub fn new(s: impl Into<String>) -> Self {
        let text = s.into();
        if text.is_empty() {
            return Self::default();
        }

        Self {
            spans: vec![Span::new(text)],
        }
    }

    pub fn styled(text: impl Into<String>, color: Color) -> Self {
        Self::with_style(text, Style::fg(color))
    }

    pub fn with_style(text: impl Into<String>, style: Style) -> Self {
        let text = text.into();
        if text.is_empty() {
            return Self::default();
        }

        Self {
            spans: vec![Span::with_style(text, style)],
        }
    }

    pub fn spans(&self) -> &[Span] {
        &self.spans
    }

    pub fn is_empty(&self) -> bool {
        self.spans.is_empty()
    }

    pub fn push_text(&mut self, text: impl Into<String>) {
        self.push_span(Span::new(text));
    }

    pub fn push_styled(&mut self, text: impl Into<String>, color: Color) {
        self.push_with_style(text, Style::fg(color));
    }

    pub fn push_with_style(&mut self, text: impl Into<String>, style: Style) {
        self.push_span(Span::with_style(text, style));
    }

    pub fn push_span(&mut self, span: Span) {
        if span.text.is_empty() {
            return;
        }

        if let Some(last) = self.spans.last_mut()
            && last.style == span.style
        {
            last.text.push_str(&span.text);
            return;
        }
        self.spans.push(span);
    }

    pub fn append_line(&mut self, other: &Line) {
        for span in &other.spans {
            self.push_span(span.clone());
        }
    }

    pub fn extend_bg_to_width(&mut self, target_width: usize) {
        let current_width = UnicodeWidthStr::width(self.plain_text().as_str());
        let pad = target_width.saturating_sub(current_width);
        if pad == 0 {
            return;
        }

        let bg = self.spans.iter().find_map(|span| span.style().bg);
        if let Some(bg) = bg {
            self.push_with_style(
                format!("{:pad$}", "", pad = pad),
                Style::default().bg_color(bg),
            );
        } else {
            self.push_text(format!("{:pad$}", "", pad = pad));
        }
    }

    pub fn to_ansi_string(&self) -> String {
        if self.spans.is_empty() {
            return String::new();
        }

        let mut out = String::new();
        let mut active_style = Style::default();

        for span in &self.spans {
            if span.style != active_style {
                emit_style_transition(&mut out, active_style, span.style);
                active_style = span.style;
            }
            out.push_str(&span.text);
        }

        if active_style != Style::default() {
            emit_style_transition(&mut out, active_style, Style::default());
        }

        out
    }

    /// Display width in terminal columns (accounts for unicode widths).
    pub fn display_width(&self) -> usize {
        soft_wrap::display_width_line(self)
    }

    /// Soft-wrap this line to fit within `width` columns.
    pub fn soft_wrap(&self, width: u16) -> Vec<Line> {
        soft_wrap::soft_wrap_line(self, width)
    }

    #[allow(dead_code)]
    pub fn plain_text(&self) -> String {
        let mut text = String::new();
        for span in &self.spans {
            text.push_str(&span.text);
        }
        text
    }
}

impl std::fmt::Display for Line {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for span in &self.spans {
            f.write_str(&span.text)?;
        }
        Ok(())
    }
}

fn push_fg_sgr(out: &mut String, color: Option<Color>) {
    let fg = color.unwrap_or(Color::Reset);
    let _ = write!(out, "{}", SetForegroundColor(fg));
}

fn push_bg_sgr(out: &mut String, color: Option<Color>) {
    let bg = color.unwrap_or(Color::Reset);
    let _ = write!(out, "{}", SetBackgroundColor(bg));
}

fn push_attr_sgr(out: &mut String, attr: Attribute) {
    let _ = write!(out, "{}", SetAttribute(attr));
}

fn emit_style_transition(out: &mut String, from: Style, to: Style) {
    // Check if any boolean attribute turned OFF — requires a full reset
    let needs_reset = (from.bold && !to.bold)
        || (from.italic && !to.italic)
        || (from.underline && !to.underline)
        || (from.dim && !to.dim)
        || (from.strikethrough && !to.strikethrough);

    if needs_reset {
        push_attr_sgr(out, Attribute::Reset);
        // After reset, re-emit all active attributes and colors on `to`
        if to.bold {
            push_attr_sgr(out, Attribute::Bold);
        }
        if to.italic {
            push_attr_sgr(out, Attribute::Italic);
        }
        if to.underline {
            push_attr_sgr(out, Attribute::Underlined);
        }
        if to.dim {
            push_attr_sgr(out, Attribute::Dim);
        }
        if to.strikethrough {
            push_attr_sgr(out, Attribute::CrossedOut);
        }
        push_fg_sgr(out, to.fg);
        push_bg_sgr(out, to.bg);
        return;
    }

    // Only turning attributes ON — emit incrementally
    if !from.bold && to.bold {
        push_attr_sgr(out, Attribute::Bold);
    }
    if !from.italic && to.italic {
        push_attr_sgr(out, Attribute::Italic);
    }
    if !from.underline && to.underline {
        push_attr_sgr(out, Attribute::Underlined);
    }
    if !from.dim && to.dim {
        push_attr_sgr(out, Attribute::Dim);
    }
    if !from.strikethrough && to.strikethrough {
        push_attr_sgr(out, Attribute::CrossedOut);
    }
    if from.fg != to.fg {
        push_fg_sgr(out, to.fg);
    }
    if from.bg != to.bg {
        push_bg_sgr(out, to.bg);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builder_style_supports_bold_and_color() {
        let mut line = Line::default();
        line.push_with_style("hot", Style::default().bold().color(Color::Red));

        let ansi = line.to_ansi_string();
        let mut bold = String::new();
        let mut red = String::new();
        let mut reset_attr = String::new();
        push_attr_sgr(&mut bold, Attribute::Bold);
        push_fg_sgr(&mut red, Some(Color::Red));
        push_attr_sgr(&mut reset_attr, Attribute::Reset);

        assert!(ansi.contains(&bold));
        assert!(ansi.contains(&red));
        assert!(ansi.contains("hot"));
        // When bold turns off, a full Reset is emitted
        assert!(ansi.contains(&reset_attr));
    }
}
