use tui::testing::TestTerminal;
use wisp::tui::{Cursor, Frame, Line, RenderContext, Renderer, RootComponent, theme::Theme};

struct StubRoot {
    lines: Vec<Line>,
    cursor: Cursor,
}

impl RootComponent for StubRoot {
    type Props = u64;

    fn props(&mut self, _context: &RenderContext) -> u64 {
        0
    }

    fn render(&self, _props: &u64, _context: &RenderContext) -> Frame {
        Frame::new(self.lines.clone(), self.cursor)
    }
}

#[test]
fn render_soft_wraps_before_diffing() {
    let mut renderer = create_renderer(3, 20);

    let mut root = StubRoot {
        lines: vec![Line::new("abcdef")],
        cursor: Cursor {
            row: 0,
            col: 5,
            is_visible: true,
        },
    };

    renderer.render(&mut root).unwrap();

    let lines = renderer.writer().get_lines();
    assert_eq!(lines[0], "abc");
    assert_eq!(lines[1], "def");
}

#[test]
fn push_to_scrollback_soft_wraps_long_lines() {
    let mut renderer = create_renderer(5, 20);

    // Render a short line first so the managed region has content
    let mut root = StubRoot {
        lines: vec![Line::new("abcde")],
        cursor: Cursor {
            row: 0,
            col: 0,
            is_visible: true,
        },
    };
    renderer.render(&mut root).unwrap();

    // Push a 10-char line at width 5 — should soft-wrap into two lines
    renderer
        .push_to_scrollback(&[Line::new("0123456789")])
        .unwrap();

    let transcript = renderer.writer().get_transcript_lines();
    assert!(
        transcript.iter().any(|l| l.contains("01234")),
        "expected wrapped first half in transcript: {transcript:?}"
    );
    assert!(
        transcript.iter().any(|l| l.contains("56789")),
        "expected wrapped second half in transcript: {transcript:?}"
    );
    // The two halves must NOT appear as one contiguous string
    assert!(
        !transcript.iter().any(|l| l.contains("0123456789")),
        "line should have been split by soft-wrap: {transcript:?}"
    );
}

#[test]
fn out_of_bounds_cursor_clamps_without_panicking() {
    let mut renderer = create_renderer(4, 20);

    let mut root = StubRoot {
        lines: vec![Line::new("a")],
        cursor: Cursor {
            row: 10,
            col: 100,
            is_visible: true,
        },
    };

    renderer.render(&mut root).unwrap();
    let lines = renderer.writer().get_lines();
    assert_eq!(lines[0], "a");
}

#[test]
fn render_flushes_overflow_to_scrollback() {
    let mut renderer = create_renderer(20, 3);

    let mut root = StubRoot {
        lines: vec![
            Line::new("L1"),
            Line::new("L2"),
            Line::new("L3"),
            Line::new("L4"),
            Line::new("L5"),
        ],
        cursor: Cursor {
            row: 4,
            col: 0,
            is_visible: true,
        },
    };

    renderer.render(&mut root).unwrap();

    // Visible frame should contain only the bottom 3 lines
    let visible = renderer.writer().get_lines();
    assert_eq!(visible[0], "L3");
    assert_eq!(visible[1], "L4");
    assert_eq!(visible[2], "L5");

    // The overflow lines (L1, L2) should be in the transcript (scrollback)
    let transcript = renderer.writer().get_transcript_lines();
    assert!(
        transcript.iter().any(|l| l == "L1"),
        "L1 should be in scrollback: {transcript:?}"
    );
    assert!(
        transcript.iter().any(|l| l == "L2"),
        "L2 should be in scrollback: {transcript:?}"
    );
}

#[test]
fn render_progressively_flushes_overflow() {
    let mut renderer = create_renderer(20, 3);

    // First render: 4 lines → 1 overflows (L1 goes to scrollback)
    let mut root = StubRoot {
        lines: vec![
            Line::new("L1"),
            Line::new("L2"),
            Line::new("L3"),
            Line::new("L4"),
        ],
        cursor: Cursor {
            row: 3,
            col: 0,
            is_visible: true,
        },
    };
    renderer.render(&mut root).unwrap();

    let transcript_after_first = renderer.writer().get_transcript_lines();
    assert!(
        transcript_after_first.iter().any(|l| l == "L1"),
        "L1 should be flushed after first render: {transcript_after_first:?}"
    );

    // Second render: 6 lines → 3 overflow. L1 already flushed, so L2 and L3 are new.
    root.lines = vec![
        Line::new("L1"),
        Line::new("L2"),
        Line::new("L3"),
        Line::new("L4"),
        Line::new("L5"),
        Line::new("L6"),
    ];
    root.cursor.row = 5;
    renderer.render(&mut root).unwrap();

    let transcript_after_second = renderer.writer().get_transcript_lines();
    assert!(
        transcript_after_second.iter().any(|l| l == "L2"),
        "L2 should be in transcript after second render: {transcript_after_second:?}"
    );
    assert!(
        transcript_after_second.iter().any(|l| l == "L3"),
        "L3 should be in transcript after second render: {transcript_after_second:?}"
    );

    // Managed (visible) frame should be the bottom 3
    let visible = renderer.writer().get_lines();
    assert_eq!(visible[0], "L4");
    assert_eq!(visible[1], "L5");
    assert_eq!(visible[2], "L6");
}

#[test]
fn push_to_scrollback_resets_flushed_count() {
    let mut renderer = create_renderer(20, 3);

    // Render 5 lines (2 overflow → L1, L2 flushed progressively)
    let mut root = StubRoot {
        lines: vec![
            Line::new("L1"),
            Line::new("L2"),
            Line::new("L3"),
            Line::new("L4"),
            Line::new("L5"),
        ],
        cursor: Cursor {
            row: 4,
            col: 0,
            is_visible: true,
        },
    };
    renderer.render(&mut root).unwrap();

    // Push to scrollback (resets flushed count internally)
    renderer
        .push_to_scrollback(&[Line::new("committed")])
        .unwrap();

    // Now render new overflow — if the counter was NOT reset, the renderer
    // would skip flushing because it thinks lines are already flushed.
    root.lines = vec![
        Line::new("A1"),
        Line::new("A2"),
        Line::new("A3"),
        Line::new("A4"),
        Line::new("A5"),
    ];
    renderer.render(&mut root).unwrap();

    let transcript = renderer.writer().get_transcript_lines();
    // A1 and A2 should have been flushed to scrollback (2 overflow lines)
    assert!(
        transcript.iter().any(|l| l == "A1"),
        "A1 should be in scrollback (proves counter was reset): {transcript:?}"
    );
    assert!(
        transcript.iter().any(|l| l == "A2"),
        "A2 should be in scrollback (proves counter was reset): {transcript:?}"
    );
}

fn create_renderer(cols: u16, rows: u16) -> Renderer<TestTerminal> {
    let terminal = TestTerminal::new(cols, rows);
    let mut renderer = Renderer::new(terminal, Theme::default());
    renderer.on_resize((cols, rows));
    renderer
}
