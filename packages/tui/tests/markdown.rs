use tui::testing::{TestTerminal, render_lines};
use tui::{Line, Theme, ViewContext, render_markdown};
use unicode_width::UnicodeWidthStr;

fn ctx() -> ViewContext {
    ViewContext::new((80, 24))
}

fn themed_ctx(theme: &Theme) -> ViewContext {
    ViewContext::new_with_theme((80, 24), theme.clone())
}

fn render(md: &str) -> TestTerminal {
    let ctx = ctx();
    render_lines(&render_markdown(md, &ctx), 80, 24)
}

fn render_themed(md: &str, theme: &Theme) -> TestTerminal {
    let ctx = themed_ctx(theme);
    render_lines(&render_markdown(md, &ctx), 80, 24)
}

fn render_tall(md: &str) -> TestTerminal {
    let ctx = ViewContext::new((80, 100));
    render_lines(&render_markdown(md, &ctx), 80, 100)
}

fn render_tall_themed(md: &str, theme: &Theme) -> TestTerminal {
    let ctx = ViewContext::new_with_theme((80, 100), theme.clone());
    render_lines(&render_markdown(md, &ctx), 80, 100)
}

fn find_row(term: &TestTerminal, text: &str) -> Option<usize> {
    term.get_lines().iter().position(|line| line.contains(text))
}

fn all_text(term: &TestTerminal) -> String {
    term.get_lines().join("\n")
}

/// Assert the joined terminal output contains every needle.
fn assert_contains_all(term: &TestTerminal, needles: &[&str]) {
    let text = all_text(term);
    for needle in needles {
        assert!(
            text.contains(needle),
            "Expected to find {needle:?} in:\n{text}"
        );
    }
}

fn row_inner_display_widths(row: &str) -> Vec<usize> {
    let segments: Vec<&str> = row.split('│').collect();
    if segments.len() < 3 {
        return Vec::new();
    }
    segments[1..segments.len() - 1]
        .iter()
        .map(|s| UnicodeWidthStr::width(*s))
        .collect()
}

#[test]
fn plain_text_passes_through() {
    assert_eq!(render("hello world").get_lines()[0], "hello world");
}

#[test]
fn heading_renders_with_prefix_and_style() {
    let term = render("# Title");
    let row = find_row(&term, "# Title").expect("heading row not found");
    assert!(term.get_lines()[row].contains("# Title"));
    assert!(term.style_of_text(row, "Title").unwrap().bold);
}

#[test]
fn inline_formatting_styles() {
    let cases: &[(&str, &str, &str, fn(&tui::Style) -> bool)] = &[
        ("some **bold** text", "some bold text", "bold", |s| s.bold),
        ("some *italic* text", "some italic text", "italic", |s| {
            s.italic
        }),
        ("some ~~struck~~ text", "some struck text", "struck", |s| {
            s.strikethrough
        }),
        (
            "***bold and italic***",
            "bold and italic",
            "bold and italic",
            |s| s.bold && s.italic,
        ),
    ];
    for (md, expected_text, styled_span, check) in cases {
        let term = render(md);
        assert_eq!(term.get_lines()[0].trim(), *expected_text, "md={md}");
        let style = term.style_of_text(0, styled_span).unwrap();
        assert!(check(&style), "style check failed for md={md}");
    }
}

#[test]
fn inline_code_has_code_style() {
    let theme = Theme::default();
    let term = render_themed("use `foo()` here", &theme);
    assert_eq!(term.get_lines()[0].trim(), "use foo() here");
    assert_eq!(
        term.style_of_text(0, "foo()").unwrap().fg,
        Some(theme.code_fg())
    );
}

#[test]
fn fenced_code_block_produces_lines() {
    assert!(all_text(&render("```rust\nfn main() {}\n```")).contains("fn main()"));
}

#[test]
fn list_items_render() {
    let cases: &[(&str, &[&str])] = &[
        (
            "- alpha\n- beta\n- gamma",
            &["- alpha", "- beta", "- gamma"],
        ),
        (
            "1. first\n2. second\n3. third",
            &["1. first", "2. second", "3. third"],
        ),
    ];
    for (md, expected) in cases {
        let term = render(md);
        let output = term.get_lines();
        for item in *expected {
            assert!(
                output.iter().any(|t| t.contains(item)),
                "Missing {item:?} in {md}"
            );
        }
    }
}

#[test]
fn blockquote_is_indented() {
    let theme = Theme::default();
    let term = render_themed("> quoted text", &theme);
    let output = term.get_lines();
    let row = find_row(&term, "quoted text").expect("quoted text row not found");
    assert!(output[row].starts_with("  quoted text"));
    let style = term.style_of_text(row, "quoted text").unwrap();
    assert_eq!(style.fg, Some(theme.blockquote()));
    assert!(!style.dim);
}

#[test]
fn horizontal_rule() {
    assert!(all_text(&render("above\n\n---\n\nbelow")).contains("───"));
}

#[test]
fn link_is_underlined() {
    let theme = Theme::default();
    let term = render_themed("click [here](https://example.com)", &theme);
    let style = term.style_of_text(0, "here").unwrap();
    assert!(style.underline);
    assert_eq!(style.fg, Some(theme.link()));
}

#[test]
fn empty_input_returns_empty() {
    assert!(render_markdown("", &ctx()).is_empty());
}

#[test]
fn multiple_paragraphs_have_spacing() {
    let term = render("para one\n\npara two");
    let output = term.get_lines();
    let last_non_empty = output.iter().rposition(|l| !l.is_empty()).unwrap_or(0);
    let non_trailing: Vec<&String> = output[..=last_non_empty].iter().collect();
    assert!(non_trailing.len() >= 3);
    assert!(non_trailing.iter().any(|t| t.is_empty()));
}

#[test]
fn unknown_language_falls_back_to_plain() {
    let theme = Theme::default();
    let term = render_themed("```nosuchlang\nsome code\n```", &theme);
    assert!(all_text(&term).contains("some code"));
    let row = find_row(&term, "some code").expect("code row not found");
    assert_eq!(
        term.style_of_text(row, "some code").unwrap().fg,
        Some(theme.code_fg())
    );
}

#[test]
fn nested_list_indents() {
    let output = render("- outer\n  - inner").get_lines();
    let inner = output.iter().find(|t| t.contains("inner")).unwrap();
    let outer = output.iter().find(|t| t.contains("outer")).unwrap();
    assert!(inner.len() - inner.trim_start().len() > outer.len() - outer.trim_start().len());
}

#[test]
fn highlight_cache_returns_consistent_results() {
    let ctx = ctx();
    let md = "```rust\nfn main() {}\n```";
    let first = render_markdown(md, &ctx);
    let second = render_markdown(md, &ctx);
    assert_eq!(first, second);

    let md2 = "```rust\nfn b() {}\n```";
    let lines2 = render_markdown(md2, &ctx);
    assert_ne!(
        first.iter().map(Line::plain_text).collect::<String>(),
        lines2.iter().map(Line::plain_text).collect::<String>(),
    );
}

#[test]
fn simple_table_renders_correctly() {
    let md =
        "| Name | Age | City |\n|------|-----|------|\n| Alice | 30 | NYC |\n| Bob | 25 | LA |";
    let term = render_tall(md);
    let output = term.get_lines();
    let text = all_text(&term);
    let non_empty: Vec<&String> = output.iter().filter(|t| !t.is_empty()).collect();

    for ch in ['┌', '┐', '┬', '┼', '┴', '├', '┤', '└', '┘', '│'] {
        assert!(text.contains(ch), "Missing table char {ch:?}");
    }
    assert_eq!(output.iter().filter(|t| t.contains('┼')).count(), 1);
    assert_eq!(non_empty.len(), 6);
    assert_contains_all(&term, &["Alice", "30", "NYC", "Bob", "25", "LA"]);
    assert!(
        !output.iter().any(|t| t.trim() == "Alice"),
        "Table content leaked to standalone line"
    );
}

#[test]
fn table_with_alignment() {
    let md = "| Left | Center | Right |\n|:-----|:------:|------:|\n| L | C | R |";
    assert_contains_all(&render_tall(md), &["Left", "Center", "Right"]);
}

#[test]
fn table_with_empty_cells() {
    let term = render_tall("| A | B | C |\n|---|---|---|\n| 1 |   | 3 |");
    let text = all_text(&term);
    assert!(text.contains('┌'));
    assert!(text.contains('1'));
    assert!(text.contains('3'));
}

#[test]
fn table_cell_inline_code_does_not_leak_line() {
    let term = render_tall("| A | B |\n|---|---|\n| `x` and **y** | z |");
    let output = term.get_lines();
    let non_empty: Vec<&String> = output.iter().filter(|t| !t.is_empty()).collect();

    assert_eq!(non_empty.len(), 5);
    assert!(
        !output.iter().any(|t| t.trim() == "x"),
        "Inline code leaked outside table"
    );
    let body_row = output
        .iter()
        .find(|t| t.starts_with('│') && t.contains("and y"))
        .expect("Expected a rendered table body row");
    assert!(body_row.contains('x'));
}

#[test]
fn table_cell_preserves_inline_styles() {
    let theme = Theme::default();
    let md = "| A | B | C |\n|---|---|---|\n| **bold** | [link](https://example.com) | `code` |";
    let term = render_tall_themed(md, &theme);

    let bold_row = find_row(&term, "bold").expect("bold row not found");
    assert!(term.style_of_text(bold_row, "bold").unwrap().bold);

    let link_row = find_row(&term, "link").expect("link row not found");
    let link_style = term.style_of_text(link_row, "link").unwrap();
    assert!(link_style.underline);
    assert_eq!(link_style.fg, Some(theme.link()));

    let code_row = find_row(&term, "code").expect("code row not found");
    assert_eq!(
        term.style_of_text(code_row, "code").unwrap().fg,
        Some(theme.code_fg())
    );
}

#[test]
fn table_unicode_alignment_uses_display_width() {
    let md = "| Left | Right |\n|------|-------|\n| a | 你 |\n| bb | 😀 |";
    let output = render_tall(md).get_lines();
    let row_texts: Vec<&String> = output.iter().filter(|t| t.starts_with('│')).collect();

    assert!(row_texts.len() >= 3);
    let expected = row_inner_display_widths(row_texts[0]);
    for row in row_texts.iter().skip(1) {
        assert_eq!(row_inner_display_widths(row), expected);
    }
}

#[test]
fn table_row_cell_count_normalization() {
    let md = "| A | B | C |\n|---|---|---|\n| 1 | 2 |\n| 3 | 4 | 5 | 6 |";
    let output = render_tall(md).get_lines();
    let row_texts: Vec<&String> = output.iter().filter(|t| t.starts_with('│')).collect();

    assert_eq!(row_texts.len(), 3);
    for row in &row_texts {
        assert_eq!(row.matches('│').count(), 4);
        assert_eq!(row_inner_display_widths(row).len(), 3);
    }
}

#[test]
fn table_in_paragraph_context() {
    let md = "Here is a table:\n\n| Item | Price |\n|------|-------|\n| Apple | $1.00 |\n| Orange | $1.50 |\n\nThat's the table.";
    assert_contains_all(
        &render_tall(md),
        &["Here is a table:", "That's the table.", "Apple", "$1.00"],
    );
}

#[test]
fn table_single_column() {
    let term = render_tall("| Value |\n|--------|\n| Hello |");
    let text = all_text(&term);
    assert!(text.contains('┌'));
    assert!(text.contains('┐'));
    assert!(text.contains("Hello"));
}
