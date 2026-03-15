use tui::testing::{render_lines, TestTerminal};
use tui::{render_markdown, Line, Theme, ViewContext};
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

fn render(md: &str) -> TestTerminal {
    let ctx = test_context();
    let lines = render_markdown(md, &ctx);
    render_lines(&lines, 80, 24)
}

fn render_with_theme(md: &str, theme: &Theme) -> TestTerminal {
    let ctx = test_context_with_theme(theme.clone());
    let lines = render_markdown(md, &ctx);
    render_lines(&lines, 80, 24)
}

fn render_tall(md: &str) -> TestTerminal {
    let ctx = ViewContext::new((80, 100));
    let lines = render_markdown(md, &ctx);
    render_lines(&lines, 80, 100)
}

fn render_tall_with_theme(md: &str, theme: &Theme) -> TestTerminal {
    let ctx = ViewContext::new_with_theme((80, 100), theme.clone());
    let lines = render_markdown(md, &ctx);
    render_lines(&lines, 80, 100)
}

/// Find the row index containing the given text in the terminal output.
fn find_row(term: &TestTerminal, text: &str) -> Option<usize> {
    term.get_lines()
        .iter()
        .position(|line| line.contains(text))
}

#[test]
fn plain_text_passes_through() {
    let term = render("hello world");
    let output = term.get_lines();
    assert_eq!(output[0], "hello world");
}

#[test]
fn heading_renders_with_prefix_and_style() {
    let term = render("# Title");
    let output = term.get_lines();
    let heading_row = find_row(&term, "# Title").expect("heading row not found");
    assert!(output[heading_row].contains("# Title"));
    let style = term.style_of_text(heading_row, "Title").unwrap();
    assert!(style.bold);
}

#[test]
fn bold_text_is_bold() {
    let term = render("some **bold** text");
    let output = term.get_lines();
    assert_eq!(output[0].trim(), "some bold text");
    let style = term.style_of_text(0, "bold").unwrap();
    assert!(style.bold);
}

#[test]
fn italic_text_is_italic() {
    let term = render("some *italic* text");
    let output = term.get_lines();
    assert_eq!(output[0].trim(), "some italic text");
    let style = term.style_of_text(0, "italic").unwrap();
    assert!(style.italic);
}

#[test]
fn strikethrough_text() {
    let term = render("some ~~struck~~ text");
    let output = term.get_lines();
    assert_eq!(output[0].trim(), "some struck text");
    let style = term.style_of_text(0, "struck").unwrap();
    assert!(style.strikethrough);
}

#[test]
fn inline_code_has_code_style() {
    let theme = test_theme();
    let term = render_with_theme("use `foo()` here", &theme);
    let output = term.get_lines();
    assert_eq!(output[0].trim(), "use foo() here");
    let style = term.style_of_text(0, "foo()").unwrap();
    assert_eq!(style.fg, Some(theme.code_fg()));
}

#[test]
fn fenced_code_block_produces_lines() {
    let md = "```rust\nfn main() {}\n```";
    let term = render(md);
    let output = term.get_lines();
    let all_text = output.join("\n");
    assert!(all_text.contains("fn main()"));
}

#[test]
fn unordered_list() {
    let md = "- alpha\n- beta\n- gamma";
    let term = render(md);
    let output = term.get_lines();
    assert!(output.iter().any(|t| t.contains("- alpha")));
    assert!(output.iter().any(|t| t.contains("- beta")));
    assert!(output.iter().any(|t| t.contains("- gamma")));
}

#[test]
fn ordered_list() {
    let md = "1. first\n2. second\n3. third";
    let term = render(md);
    let output = term.get_lines();
    assert!(output.iter().any(|t| t.contains("1. first")));
    assert!(output.iter().any(|t| t.contains("2. second")));
    assert!(output.iter().any(|t| t.contains("3. third")));
}

#[test]
fn blockquote_is_indented() {
    let md = "> quoted text";
    let theme = test_theme();
    let term = render_with_theme(md, &theme);
    let output = term.get_lines();
    let row = find_row(&term, "quoted text").expect("quoted text row not found");
    assert!(output[row].starts_with("  quoted text"));
    let style = term.style_of_text(row, "quoted text").unwrap();
    assert_eq!(style.fg, Some(theme.blockquote()));
    assert!(!style.dim);
}

#[test]
fn horizontal_rule() {
    let md = "above\n\n---\n\nbelow";
    let term = render(md);
    let output = term.get_lines();
    let all_text = output.join("\n");
    assert!(all_text.contains("───"));
}

#[test]
fn link_is_underlined() {
    let md = "click [here](https://example.com)";
    let theme = test_theme();
    let term = render_with_theme(md, &theme);
    let style = term.style_of_text(0, "here").unwrap();
    assert!(style.underline);
    assert_eq!(style.fg, Some(theme.link()));
}

#[test]
fn empty_input_returns_empty() {
    let lines = render_markdown("", &test_context());
    assert!(lines.is_empty());
}

#[test]
fn multiple_paragraphs_have_spacing() {
    let md = "para one\n\npara two";
    let term = render(md);
    let output = term.get_lines();
    // Should have at least 3 lines: "para one", empty, "para two"
    let non_trailing: Vec<&String> = {
        let last_non_empty = output.iter().rposition(|l| !l.is_empty()).unwrap_or(0);
        output[..=last_non_empty].iter().collect()
    };
    assert!(non_trailing.len() >= 3);
    assert!(non_trailing.iter().any(|t| t.is_empty()));
}

#[test]
fn nested_bold_italic() {
    let md = "***bold and italic***";
    let term = render(md);
    let style = term.style_of_text(0, "bold and italic").unwrap();
    assert!(style.bold);
    assert!(style.italic);
}

#[test]
fn unknown_language_falls_back_to_plain() {
    let md = "```nosuchlang\nsome code\n```";
    let theme = test_theme();
    let term = render_with_theme(md, &theme);
    let output = term.get_lines();
    let all_text = output.join("\n");
    assert!(all_text.contains("some code"));
    let row = find_row(&term, "some code").expect("code row not found");
    let style = term.style_of_text(row, "some code").unwrap();
    assert_eq!(style.fg, Some(theme.code_fg()));
}

#[test]
fn nested_list_indents() {
    let md = "- outer\n  - inner";
    let term = render(md);
    let output = term.get_lines();
    assert!(output.iter().any(|t| t.contains("outer")));
    assert!(output.iter().any(|t| t.contains("inner")));
    let inner = output.iter().find(|t| t.contains("inner")).unwrap();
    let outer = output.iter().find(|t| t.contains("outer")).unwrap();
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
    assert_ne!(
        lines1.iter().map(Line::plain_text).collect::<String>(),
        lines2.iter().map(Line::plain_text).collect::<String>(),
    );
}

#[test]
fn simple_table_renders_correctly() {
    let md =
        "| Name | Age | City |\n|------|-----|------|\n| Alice | 30 | NYC |\n| Bob | 25 | LA |";
    let term = render_tall(md);
    let output = term.get_lines();
    let all_text = output.join("\n");
    let non_empty_lines: Vec<&String> = output.iter().filter(|t| !t.is_empty()).collect();

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
    assert_eq!(output.iter().filter(|t| t.contains('┼')).count(), 1);
    assert_eq!(non_empty_lines.len(), 6);

    assert!(all_text.contains("Alice"));
    assert!(all_text.contains("30"));
    assert!(all_text.contains("NYC"));
    assert!(all_text.contains("Bob"));
    assert!(all_text.contains("25"));
    assert!(all_text.contains("LA"));
    assert!(
        !output.iter().any(|t| t.trim() == "Alice"),
        "Table content leaked to standalone line: {output:?}"
    );
}

#[test]
fn table_with_alignment() {
    let md = "| Left | Center | Right |\n|:-----|:------:|------:|\n| L | C | R |";
    let term = render_tall(md);
    let output = term.get_lines();
    let all_text = output.join("\n");

    assert!(all_text.contains("Left"));
    assert!(all_text.contains("Center"));
    assert!(all_text.contains("Right"));
}

#[test]
fn table_with_empty_cells() {
    let md = "| A | B | C |\n|---|---|---|\n| 1 |   | 3 |";
    let term = render_tall(md);
    let output = term.get_lines();
    let all_text = output.join("\n");

    assert!(all_text.contains('┌'));
    assert!(all_text.contains('1'));
    assert!(all_text.contains('3'));
}

#[test]
fn table_cell_inline_code_does_not_leak_line() {
    let md = "| A | B |\n|---|---|\n| `x` and **y** | z |";
    let term = render_tall(md);
    let output = term.get_lines();
    let non_empty_lines: Vec<&String> = output.iter().filter(|t| !t.is_empty()).collect();

    assert_eq!(non_empty_lines.len(), 5);
    assert!(
        !output.iter().any(|t| t.trim() == "x"),
        "Inline code leaked outside table: {output:?}"
    );
    let body_row = output
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
    let term = render_tall_with_theme(md, &theme);
    let output = term.get_lines();

    let bold_row = find_row(&term, "bold").expect("bold row not found");
    let bold_style = term.style_of_text(bold_row, "bold").unwrap();
    assert!(bold_style.bold);

    let link_row = output
        .iter()
        .position(|l| l.contains("link"))
        .expect("link row not found");
    let link_style = term.style_of_text(link_row, "link").unwrap();
    assert!(link_style.underline);
    assert_eq!(link_style.fg, Some(theme.link()));

    let code_row = output
        .iter()
        .position(|l| l.contains("code"))
        .expect("code row not found");
    let code_style = term.style_of_text(code_row, "code").unwrap();
    assert_eq!(code_style.fg, Some(theme.code_fg()));
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
    let term = render_tall(md);
    let output = term.get_lines();
    let row_texts: Vec<&String> = output
        .iter()
        .filter(|text| text.starts_with('│'))
        .collect();

    assert!(row_texts.len() >= 3);
    let expected_widths = row_inner_display_widths(row_texts[0]);
    for row in row_texts.iter().skip(1) {
        assert_eq!(row_inner_display_widths(row), expected_widths);
    }
}

#[test]
fn table_row_cell_count_normalization() {
    let md = "| A | B | C |\n|---|---|---|\n| 1 | 2 |\n| 3 | 4 | 5 | 6 |";
    let term = render_tall(md);
    let output = term.get_lines();
    let row_texts: Vec<&String> = output
        .iter()
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
    let term = render_tall(md);
    let output = term.get_lines();
    let all_text = output.join("\n");

    assert!(all_text.contains("Here is a table:"));
    assert!(all_text.contains("That's the table."));
    assert!(all_text.contains("Apple"));
    assert!(all_text.contains("$1.00"));
}

#[test]
fn table_single_column() {
    let md = "| Value |\n|--------|\n| Hello |";
    let term = render_tall(md);
    let output = term.get_lines();
    let all_text = output.join("\n");

    assert!(all_text.contains('┌'));
    assert!(all_text.contains('┐'));
    assert!(all_text.contains("Hello"));
}
