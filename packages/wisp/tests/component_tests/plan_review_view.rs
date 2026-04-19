use tui::testing::{key, render_component};
use tui::{Component, Event, KeyCode, ViewContext};
use wisp::components::app::{PlanReviewAction, PlanReviewInput, PlanReviewMode};
use wisp::components::plan_review::PlanDocument;

fn make_mode(markdown: &str) -> PlanReviewMode {
    PlanReviewMode::new(PlanReviewInput {
        title: "Review /tmp/test-plan.md".to_string(),
        document: PlanDocument::parse("/tmp/test-plan.md", markdown),
    })
}

fn render_mode(mode: &mut PlanReviewMode, width: u16, height: u16) {
    let ctx = ViewContext::new((width, height));
    let _ = mode.render(&ctx);
}

async fn send_keys_with_render(mode: &mut PlanReviewMode, codes: &[KeyCode], width: u16, height: u16) {
    for &code in codes {
        render_mode(mode, width, height);
        mode.on_event(&Event::Key(key(code))).await;
    }
}

#[tokio::test]
async fn cursor_navigation_moves_between_blocks() {
    let mut mode = make_mode("# One\n- line_one\n- line_two\n- line_three");
    render_mode(&mut mode, 80, 24);

    assert_eq!(mode.current_anchor_line_no(), 1);

    send_keys_with_render(&mut mode, &[KeyCode::Char('j')], 80, 24).await;
    assert_eq!(mode.current_anchor_line_no(), 2);

    send_keys_with_render(&mut mode, &[KeyCode::Char('G')], 80, 24).await;
    assert_eq!(mode.current_anchor_line_no(), 4);

    send_keys_with_render(&mut mode, &[KeyCode::Char('g')], 80, 24).await;
    assert_eq!(mode.current_anchor_line_no(), 1);
}

#[tokio::test]
async fn outline_selection_owns_navigation_until_enter_jumps_document() {
    let mut mode = make_mode("# One\n\nbody first line\nbody second line\n\n## Two\n\nmore");
    render_mode(&mut mode, 80, 24);

    send_keys_with_render(&mut mode, &[KeyCode::Char('h'), KeyCode::Char('j')], 80, 24).await;
    assert_eq!(mode.current_anchor_line_no(), 1, "moving the outline should not move the document cursor");

    send_keys_with_render(&mut mode, &[KeyCode::Enter], 80, 24).await;
    assert_eq!(
        mode.current_anchor_line_no(),
        6,
        "enter should jump the document cursor to the selected outline section"
    );
}

#[tokio::test]
async fn plan_review_uses_shared_inline_styling() {
    let mut mode = make_mode("# Intro\nThis has **bold**, *italic*, `code`, and [link](https://example.com).");
    let theme_ctx = ViewContext::new((120, 12));
    let terminal = render_component(|ctx| mode.render(ctx), 120, 12);
    let lines = terminal.get_lines();
    let row = lines
        .iter()
        .position(|line| line.contains("This has bold, italic, code, and link."))
        .expect("styled line should render without markdown markers");

    assert!(terminal.style_of_text(row, "bold").unwrap().bold);
    assert!(terminal.style_of_text(row, "italic").unwrap().italic);
    assert_eq!(terminal.style_of_text(row, "code").unwrap().fg, Some(theme_ctx.theme.code_fg()));
    let link_style = terminal.style_of_text(row, "link").unwrap();
    assert!(link_style.underline);
    assert_eq!(link_style.fg, Some(theme_ctx.theme.link()));
}

#[tokio::test]
async fn plan_body_soft_wraps_long_markdown_lines() {
    let long_line = "x".repeat(140);
    let markdown = format!("# Intro\n\n{long_line}\n\nshort_tail");
    let mut mode = make_mode(&markdown);

    send_keys_with_render(&mut mode, &[KeyCode::Char('j')], 50, 12).await;

    let terminal = render_component(|ctx| mode.render(ctx), 50, 12);
    let lines = terminal.get_lines();
    let wrapped_rows = lines.iter().filter(|line| line.contains("xxxx")).count();

    assert!(wrapped_rows > 1, "expected long markdown line to soft wrap, got lines: {lines:?}");
}

#[tokio::test]
async fn inline_comment_renders_below_wrapped_anchor_block() {
    let long_line = "x".repeat(140);
    let markdown = format!("# Intro\n\n{long_line}\n\nshort_tail");
    let mut mode = make_mode(&markdown);

    send_keys_with_render(
        &mut mode,
        &[
            KeyCode::Char('j'),
            KeyCode::Char('c'),
            KeyCode::Char('n'),
            KeyCode::Char('o'),
            KeyCode::Char('t'),
            KeyCode::Char('e'),
            KeyCode::Enter,
        ],
        80,
        20,
    )
    .await;

    let terminal = render_component(|ctx| mode.render(ctx), 80, 20);
    let lines = terminal.get_lines();

    let first_long_row = lines.iter().position(|line| line.contains("xxxx")).expect("long line should render");
    let last_long_row = lines.iter().rposition(|line| line.contains("xxxx")).expect("long line should render");
    let comment_row = lines.iter().position(|line| line.contains("note")).expect("comment should render");
    let tail_row = lines.iter().position(|line| line.contains("short_tail")).expect("tail line should render");

    assert!(last_long_row > first_long_row, "expected long anchor line to wrap, got lines: {lines:?}");
    assert!(comment_row > last_long_row, "comment should render after wrapped anchor block");
    assert!(tail_row > comment_row, "following block should remain below the comment");
}

#[tokio::test]
async fn inline_comment_and_draft_render_below_their_anchor_blocks() {
    let mut mode = make_mode("# Intro\n\nline_one\n\nline_two\n\nline_three");

    send_keys_with_render(
        &mut mode,
        &[
            KeyCode::Char('j'),
            KeyCode::Char('c'),
            KeyCode::Char('f'),
            KeyCode::Char('i'),
            KeyCode::Char('r'),
            KeyCode::Char('s'),
            KeyCode::Char('t'),
            KeyCode::Enter,
            KeyCode::Char('j'),
            KeyCode::Char('j'),
            KeyCode::Char('c'),
            KeyCode::Char('d'),
            KeyCode::Char('r'),
            KeyCode::Char('a'),
            KeyCode::Char('f'),
            KeyCode::Char('t'),
        ],
        100,
        22,
    )
    .await;

    let terminal = render_component(|ctx| mode.render(ctx), 100, 22);
    let lines = terminal.get_lines();

    let line_one_row = lines.iter().position(|line| line.contains("line_one")).expect("line_one should render");
    let submitted_row = lines.iter().position(|line| line.contains("first")).expect("submitted comment should render");
    let line_two_row = lines.iter().position(|line| line.contains("line_two")).expect("line_two should render");
    let line_three_row = lines.iter().position(|line| line.contains("line_three")).expect("line_three should render");
    let draft_row = lines.iter().position(|line| line.contains("draft")).expect("draft comment should render");

    assert!(submitted_row > line_one_row);
    assert!(line_two_row > submitted_row);
    assert!(line_three_row > line_two_row);
    assert!(draft_row > line_three_row);
}

#[tokio::test]
async fn submitted_comment_on_last_block_stays_visible_at_bottom_of_viewport() {
    let mut mode = make_mode("# Intro\n\nline_one\n\nline_two\n\nline_three");

    send_keys_with_render(
        &mut mode,
        &[KeyCode::Char('G'), KeyCode::Char('c'), KeyCode::Char('h'), KeyCode::Char('i'), KeyCode::Enter],
        100,
        7,
    )
    .await;

    let terminal = render_component(|ctx| mode.render(ctx), 100, 7);
    let lines = terminal.get_lines();

    assert!(lines.iter().any(|line| line.contains("line_three")), "cursor line should remain visible");
    assert!(lines.iter().any(|line| line.contains("hi")), "comment text should be visible");
    assert!(lines.iter().any(|line| line.contains('└')), "comment bottom border should be visible");
}

#[tokio::test]
async fn wrapped_plan_lines_navigate_by_block() {
    let long_line = "x".repeat(140);
    let markdown = format!("# Intro\n\n{long_line}\n\nshort_tail");
    let mut mode = make_mode(&markdown);

    send_keys_with_render(&mut mode, &[KeyCode::Char('j'), KeyCode::Char('j')], 50, 12).await;

    assert_eq!(mode.current_anchor_line_no(), 5, "second j should jump past the wrapped paragraph to the next block");
}

#[tokio::test]
async fn wrapped_plan_lines_highlight_only_the_active_visual_row() {
    let long_line = "x".repeat(140);
    let markdown = format!("# Intro\n\n{long_line}\n\nshort_tail");
    let mut mode = make_mode(&markdown);

    send_keys_with_render(&mut mode, &[KeyCode::Char('j')], 50, 12).await;

    let ctx = ViewContext::new((50, 12));
    let terminal = render_component(|render_ctx| mode.render(render_ctx), 50, 12);
    let lines = terminal.get_lines();
    let highlight_bg = ctx.theme.highlight_bg();
    let highlighted_wrapped_rows = lines
        .iter()
        .enumerate()
        .filter(|(row, line)| {
            line.contains("xxxx")
                && line.find('x').is_some_and(|col| terminal.get_style_at(*row, col).bg == Some(highlight_bg))
        })
        .count();

    assert_eq!(highlighted_wrapped_rows, 1, "expected exactly one wrapped visual row to be highlighted");
}

#[tokio::test]
async fn request_changes_feedback_keys_to_block_source_lines() {
    let mut mode = make_mode("# Intro\n\nfirst line\n\nsecond line\n\n## Details\n\nmore");

    send_keys_with_render(
        &mut mode,
        &[
            KeyCode::Char('j'),
            KeyCode::Char('j'),
            KeyCode::Char('c'),
            KeyCode::Char('f'),
            KeyCode::Char('i'),
            KeyCode::Char('x'),
            KeyCode::Enter,
        ],
        80,
        24,
    )
    .await;

    let deny_action = mode
        .on_event(&Event::Key(key(KeyCode::Char('r'))))
        .await
        .and_then(|mut msgs| msgs.pop())
        .expect("deny should emit an action");
    let PlanReviewAction::RequestChanges { feedback } = deny_action else {
        panic!("expected request changes action");
    };

    assert!(feedback.contains("Line 5"), "feedback should use the block's starting source line: {feedback}");
    assert!(feedback.contains("`second line`"), "feedback should quote the original source text: {feedback}");
}

#[tokio::test]
async fn approve_request_changes_and_cancel_emit_expected_actions() {
    let mut approve_mode = make_mode("# Intro\nline_one");
    let approve_action = approve_mode
        .on_event(&Event::Key(key(KeyCode::Char('a'))))
        .await
        .and_then(|mut msgs| msgs.pop())
        .expect("approve should emit an action");
    assert!(matches!(approve_action, PlanReviewAction::Approve));

    let mut deny_mode = make_mode("# Intro\n\nline_one");
    send_keys_with_render(
        &mut deny_mode,
        &[
            KeyCode::Char('j'),
            KeyCode::Char('c'),
            KeyCode::Char('n'),
            KeyCode::Char('e'),
            KeyCode::Char('e'),
            KeyCode::Char('d'),
            KeyCode::Enter,
        ],
        80,
        24,
    )
    .await;
    let deny_action = deny_mode
        .on_event(&Event::Key(key(KeyCode::Char('r'))))
        .await
        .and_then(|mut msgs| msgs.pop())
        .expect("deny should emit an action");
    let PlanReviewAction::RequestChanges { feedback } = deny_action else {
        panic!("expected request changes action");
    };
    assert!(feedback.contains("need"));

    let mut deny_without_comments_mode = make_mode("# Intro\nline_one");
    let fallback_action = deny_without_comments_mode
        .on_event(&Event::Key(key(KeyCode::Char('r'))))
        .await
        .and_then(|mut msgs| msgs.pop())
        .expect("deny should emit an action");
    let PlanReviewAction::RequestChanges { feedback } = fallback_action else {
        panic!("expected request changes action");
    };
    assert!(feedback.contains("no inline comments"));

    let mut cancel_mode = make_mode("# Intro\nline_one");
    let cancel_action = cancel_mode
        .on_event(&Event::Key(key(KeyCode::Esc)))
        .await
        .and_then(|mut msgs| msgs.pop())
        .expect("cancel should emit an action");
    assert!(matches!(cancel_action, PlanReviewAction::Cancel));
}
