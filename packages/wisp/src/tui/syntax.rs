use std::sync::LazyLock;

use crossterm::style::Color;
use syntect::highlighting::FontStyle;
use syntect::parsing::{SyntaxReference, SyntaxSet};

use super::screen::Style;

static SYNTAX_SET: LazyLock<SyntaxSet> = LazyLock::new(SyntaxSet::load_defaults_newlines);

pub(crate) fn syntax_set() -> &'static SyntaxSet {
    &SYNTAX_SET
}

pub(crate) fn find_syntax_by_token(token: &str) -> Option<&'static SyntaxReference> {
    if token.is_empty() {
        return None;
    }
    syntax_set().find_syntax_by_token(token)
}

pub(crate) fn find_syntax_for_hint(hint: &str) -> Option<&'static SyntaxReference> {
    if hint.is_empty() {
        return None;
    }

    syntax_set()
        .find_syntax_by_extension(hint)
        .or_else(|| syntax_set().find_syntax_by_token(hint))
}

pub(crate) fn syntect_to_wisp_style(s: syntect::highlighting::Style) -> Style {
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
