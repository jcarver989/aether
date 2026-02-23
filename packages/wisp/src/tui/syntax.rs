use std::sync::LazyLock;

use crossterm::style::Color;
use syntect::highlighting::{FontStyle, ScopeSelectors, StyleModifier, ThemeItem, ThemeSettings};
use syntect::parsing::{SyntaxReference, SyntaxSet};

use super::screen::Style;

pub(crate) struct SyntectState {
    pub(crate) syntax_set: SyntaxSet,
    pub(crate) theme: syntect::highlighting::Theme,
}

pub(crate) static SYNTECT: LazyLock<SyntectState> = LazyLock::new(|| SyntectState {
    syntax_set: SyntaxSet::load_defaults_newlines(),
    theme: build_ayu_dark_theme(),
});

pub(crate) fn find_syntax_by_token(token: &str) -> Option<&'static SyntaxReference> {
    if token.is_empty() {
        return None;
    }
    SYNTECT.syntax_set.find_syntax_by_token(token)
}

pub(crate) fn find_syntax_for_hint(hint: &str) -> Option<&'static SyntaxReference> {
    if hint.is_empty() {
        return None;
    }

    SYNTECT
        .syntax_set
        .find_syntax_by_extension(hint)
        .or_else(|| SYNTECT.syntax_set.find_syntax_by_token(hint))
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
            rule(
                "string.regexp, constant.character, constant.other",
                0x95E6CB,
            ),
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
            rule(
                "meta.decorator variable.other, meta.decorator punctuation.decorator, storage.type.annotation, variable.annotation, punctuation.definition.annotation",
                0xE6B673,
            ),
            rule("invalid", 0xD95757),
        ],
    }
}
