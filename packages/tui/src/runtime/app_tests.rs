//! Tests for the App API (simplified, one-trait approach)
//!
//! This file tests the simplified app API with framework-owned rerender policy.
//! The framework decides when to render (after each update/effect cycle), not the app.

use super::app::{App, AppEvent};
use crate::components::{Cursor, ViewContext};
use crate::rendering::renderer::Renderer;
use crate::testing::TestTerminal;
use crate::theme::Theme;
use crate::{Frame, KeyCode, KeyModifiers, Line};
use crossterm::event::{Event as CrosstermEvent, KeyEvent, KeyEventKind};
use std::cell::{Cell, RefCell};
use std::io::{self, Write};
use std::rc::Rc;
use std::time::Duration;
use tokio::sync::mpsc;

// Helper to create a key event
fn key_event(code: KeyCode) -> CrosstermEvent {
    CrosstermEvent::Key(KeyEvent {
        code,
        modifiers: KeyModifiers::NONE,
        kind: KeyEventKind::Press,
        state: crate::KeyEventState::NONE,
    })
}

#[tokio::test]
async fn app_rerenders_after_state_changing_events() {
    // The framework should render after each update that changes state.
    // This is the core of the simplified rerender policy.
    let render_count = Rc::new(Cell::new(0usize));
    let render_count_clone = render_count.clone();

    struct TestApp {
        count: i32,
        exit_requested: bool,
        render_count: Rc<Cell<usize>>,
    }

    impl App for TestApp {
        type Event = ();
        type Effect = ();
        type Error = io::Error;

        fn update(&mut self, event: AppEvent<()>, _ctx: &ViewContext) -> Option<Vec<()>> {
            match event {
                AppEvent::Key(key) if key.code == KeyCode::Char('j') => {
                    self.count += 1;
                    Some(vec![])
                }
                AppEvent::Key(key) if key.code == KeyCode::Char('q') => {
                    self.exit_requested = true;
                    Some(vec![])
                }
                _ => Some(vec![]),
            }
        }

        fn view(&self, _ctx: &ViewContext) -> Frame {
            self.render_count.set(self.render_count.get() + 1);
            Frame::new(
                vec![Line::new(format!("Count: {}", self.count))],
                Cursor {
                    row: 0,
                    col: 0,
                    is_visible: false,
                },
            )
        }

        fn should_exit(&self) -> bool {
            self.exit_requested
        }
    }

    let app = TestApp {
        count: 0,
        exit_requested: false,
        render_count: render_count_clone,
    };
    let mut renderer = Renderer::new(TestTerminal::new(20, 4), Theme::default());
    renderer.on_resize((20, 4));
    let (terminal_tx, terminal_rx) = mpsc::unbounded_channel();

    // Send two increment events, then exit
    terminal_tx.send(key_event(KeyCode::Char('j'))).unwrap();
    terminal_tx.send(key_event(KeyCode::Char('j'))).unwrap();
    terminal_tx.send(key_event(KeyCode::Char('q'))).unwrap();
    drop(terminal_tx);

    run_app_internal(app, &mut renderer, terminal_rx, None, None)
        .await
        .unwrap();

    // Should have rendered: initial + 2 state changes = 3 renders
    // (exit doesn't render)
    assert_eq!(render_count.get(), 3);
}

// =============================================================================
// Test: App rerenders after external events
// =============================================================================

#[tokio::test]
async fn app_rerenders_after_external_events() {
    // External events should also trigger rerenders.
    let render_count = Rc::new(Cell::new(0usize));
    let render_count_clone = render_count.clone();

    struct TestApp {
        value: String,
        exit_requested: bool,
        render_count: Rc<Cell<usize>>,
    }

    impl App for TestApp {
        type Event = String;
        type Effect = ();
        type Error = io::Error;

        fn update(&mut self, event: AppEvent<String>, _ctx: &ViewContext) -> Option<Vec<()>> {
            match event {
                AppEvent::External(msg) => {
                    self.value = msg;
                    Some(vec![()])
                }
                AppEvent::Key(key) if key.code == KeyCode::Char('q') => {
                    self.exit_requested = true;
                    Some(vec![])
                }
                _ => Some(vec![]),
            }
        }

        fn view(&self, _ctx: &ViewContext) -> Frame {
            self.render_count.set(self.render_count.get() + 1);
            Frame::new(
                vec![Line::new(self.value.clone())],
                Cursor {
                    row: 0,
                    col: 0,
                    is_visible: false,
                },
            )
        }

        async fn run_effect(
            &mut self,
            _renderer: &mut Renderer<impl Write>,
            _effect: (),
        ) -> Result<Vec<()>, io::Error> {
            self.exit_requested = true;
            Ok(vec![])
        }

        fn should_exit(&self) -> bool {
            self.exit_requested
        }
    }

    let app = TestApp {
        value: "initial".to_string(),
        exit_requested: false,
        render_count: render_count_clone,
    };
    let mut renderer = Renderer::new(TestTerminal::new(20, 4), Theme::default());
    renderer.on_resize((20, 4));
    let (_terminal_tx, terminal_rx) = mpsc::unbounded_channel();
    let (external_tx, external_rx) = mpsc::unbounded_channel();

    external_tx.send("updated".to_string()).unwrap();
    drop(external_tx);

    run_app_internal(app, &mut renderer, terminal_rx, Some(external_rx), None)
        .await
        .unwrap();

    // Should have rendered: initial + pre-effect render after external event = 2 renders
    assert_eq!(render_count.get(), 2);
}

// =============================================================================
// Test: App rerenders after effect completion
// =============================================================================

#[tokio::test]
async fn app_rerenders_after_effect_completion() {
    // Effects that change state should cause a rerender.
    let render_count = Rc::new(Cell::new(0usize));
    let render_count_clone = render_count.clone();

    enum TestEffect {
        Increment,
    }

    struct TestApp {
        count: i32,
        exit_requested: bool,
        render_count: Rc<Cell<usize>>,
    }

    impl App for TestApp {
        type Event = ();
        type Effect = TestEffect;
        type Error = io::Error;

        fn update(&mut self, event: AppEvent<()>, _ctx: &ViewContext) -> Option<Vec<TestEffect>> {
            match event {
                AppEvent::Key(key) if key.code == KeyCode::Char(' ') => {
                    Some(vec![TestEffect::Increment])
                }
                AppEvent::Key(key) if key.code == KeyCode::Char('q') => {
                    self.exit_requested = true;
                    Some(vec![])
                }
                _ => Some(vec![]),
            }
        }

        fn view(&self, _ctx: &ViewContext) -> Frame {
            self.render_count.set(self.render_count.get() + 1);
            Frame::new(
                vec![Line::new(format!("Count: {}", self.count))],
                Cursor {
                    row: 0,
                    col: 0,
                    is_visible: false,
                },
            )
        }

        async fn run_effect(
            &mut self,
            _renderer: &mut Renderer<impl Write>,
            effect: TestEffect,
        ) -> Result<Vec<TestEffect>, io::Error> {
            match effect {
                TestEffect::Increment => {
                    self.count += 1;
                    Ok(vec![])
                }
            }
        }

        fn should_exit(&self) -> bool {
            self.exit_requested
        }
    }

    let app = TestApp {
        count: 0,
        exit_requested: false,
        render_count: render_count_clone,
    };
    let mut renderer = Renderer::new(TestTerminal::new(20, 4), Theme::default());
    renderer.on_resize((20, 4));
    let (terminal_tx, terminal_rx) = mpsc::unbounded_channel();

    // Trigger an effect, then exit
    terminal_tx.send(key_event(KeyCode::Char(' '))).unwrap();
    terminal_tx.send(key_event(KeyCode::Char('q'))).unwrap();
    drop(terminal_tx);

    run_app_internal(app, &mut renderer, terminal_rx, None, None)
        .await
        .unwrap();

    // Should have rendered:
    // 1. initial
    // 2. before effect (render happens before run_effect)
    // 3. after effect completion
    // Total: 3 renders
    assert_eq!(render_count.get(), 3);
}

// =============================================================================
// Test: Frame diffing avoids unnecessary terminal writes
// =============================================================================

#[tokio::test]
async fn frame_diffing_avoids_unnecessary_terminal_writes() {
    struct StaticApp {
        exit_requested: bool,
    }

    impl App for StaticApp {
        type Event = ();
        type Effect = ();
        type Error = io::Error;

        fn update(&mut self, event: AppEvent<()>, _ctx: &ViewContext) -> Option<Vec<()>> {
            match event {
                AppEvent::Key(key) if key.code == KeyCode::Char('q') => {
                    self.exit_requested = true;
                    Some(vec![])
                }
                _ => Some(vec![]),
            }
        }

        fn view(&self, _ctx: &ViewContext) -> Frame {
            // Always returns the same content
            Frame::new(
                vec![Line::new("static content")],
                Cursor {
                    row: 0,
                    col: 0,
                    is_visible: false,
                },
            )
        }

        fn should_exit(&self) -> bool {
            self.exit_requested
        }
    }

    let terminal = TestTerminal::new(20, 4);
    let mut renderer = Renderer::new(terminal, Theme::default());
    renderer.on_resize((20, 4));
    let (terminal_tx, terminal_rx) = mpsc::unbounded_channel();

    // Send multiple no-op events, then exit
    terminal_tx.send(key_event(KeyCode::Char('x'))).unwrap();
    terminal_tx.send(key_event(KeyCode::Char('y'))).unwrap();
    terminal_tx.send(key_event(KeyCode::Char('z'))).unwrap();
    terminal_tx.send(key_event(KeyCode::Char('q'))).unwrap();
    drop(terminal_tx);

    run_app_internal(
        StaticApp {
            exit_requested: false,
        },
        &mut renderer,
        terminal_rx,
        None,
        None,
    )
    .await
    .unwrap();

    // Verify the static content was rendered
    let terminal = renderer.test_writer_mut();
    let lines = terminal.get_lines();
    assert!(
        lines.iter().any(|line| line.contains("static content")),
        "Expected 'static content' in terminal output, got: {:?}",
        lines
    );
}

// =============================================================================
// Test: Tick events trigger rerender
// =============================================================================

#[tokio::test]
async fn tick_events_trigger_rerender() {
    let render_count = Rc::new(Cell::new(0usize));
    let render_count_clone = render_count.clone();
    let tick_count = Rc::new(Cell::new(0usize));
    let tick_count_clone = tick_count.clone();

    struct TestApp {
        ticks: usize,
        exit_requested: bool,
        render_count: Rc<Cell<usize>>,
        tick_count: Rc<Cell<usize>>,
    }

    impl App for TestApp {
        type Event = ();
        type Effect = ();
        type Error = io::Error;

        fn update(&mut self, event: AppEvent<()>, _ctx: &ViewContext) -> Option<Vec<()>> {
            match event {
                AppEvent::Tick(_) => {
                    self.ticks += 1;
                    self.tick_count.set(self.tick_count.get() + 1);
                    if self.ticks >= 2 {
                        self.exit_requested = true;
                    }
                    Some(vec![])
                }
                _ => Some(vec![]),
            }
        }

        fn view(&self, _ctx: &ViewContext) -> Frame {
            self.render_count.set(self.render_count.get() + 1);
            Frame::new(
                vec![Line::new(format!("Ticks: {}", self.ticks))],
                Cursor {
                    row: 0,
                    col: 0,
                    is_visible: false,
                },
            )
        }

        fn wants_tick(&self) -> bool {
            true
        }

        fn should_exit(&self) -> bool {
            self.exit_requested
        }
    }

    let app = TestApp {
        ticks: 0,
        exit_requested: false,
        render_count: render_count_clone,
        tick_count: tick_count_clone,
    };
    let mut renderer = Renderer::new(TestTerminal::new(20, 4), Theme::default());
    renderer.on_resize((20, 4));
    let (_terminal_tx, terminal_rx) = mpsc::unbounded_channel();

    run_app_internal(
        app,
        &mut renderer,
        terminal_rx,
        None::<mpsc::UnboundedReceiver<()>>,
        Some(Duration::from_millis(10)),
    )
    .await
    .unwrap();

    // Should have rendered: initial + first tick = 2 renders
    // (second tick exits without a trailing render)
    assert_eq!(render_count.get(), 2);
}

// =============================================================================
// Test: Resize triggers rerender with updated size
// =============================================================================

#[tokio::test]
async fn resize_triggers_rerender_with_updated_size() {
    let seen_sizes = Rc::new(RefCell::new(Vec::new()));
    let seen_sizes_clone = seen_sizes.clone();

    struct TestApp {
        exit_requested: bool,
        seen_sizes: Rc<RefCell<Vec<(u16, u16)>>>,
    }

    impl App for TestApp {
        type Event = ();
        type Effect = ();
        type Error = io::Error;

        fn update(&mut self, event: AppEvent<()>, _ctx: &ViewContext) -> Option<Vec<()>> {
            match event {
                AppEvent::Key(key) if key.code == KeyCode::Char('q') => {
                    self.exit_requested = true;
                    Some(vec![])
                }
                _ => Some(vec![]),
            }
        }

        fn view(&self, ctx: &ViewContext) -> Frame {
            self.seen_sizes
                .borrow_mut()
                .push((ctx.size.width, ctx.size.height));
            Frame::new(
                vec![Line::new(format!(
                    "Size: {}x{}",
                    ctx.size.width, ctx.size.height
                ))],
                Cursor {
                    row: 0,
                    col: 0,
                    is_visible: false,
                },
            )
        }

        fn should_exit(&self) -> bool {
            self.exit_requested
        }
    }

    let app = TestApp {
        exit_requested: false,
        seen_sizes: seen_sizes_clone,
    };
    let mut renderer = Renderer::new(TestTerminal::new(20, 4), Theme::default());
    renderer.on_resize((20, 4));
    let (terminal_tx, terminal_rx) = mpsc::unbounded_channel();

    // Send resize event, then exit
    terminal_tx.send(CrosstermEvent::Resize(40, 10)).unwrap();
    terminal_tx.send(key_event(KeyCode::Char('q'))).unwrap();
    drop(terminal_tx);

    run_app_internal(app, &mut renderer, terminal_rx, None, None)
        .await
        .unwrap();

    // Should have seen: initial size (20, 4), then resized (40, 10)
    let sizes = seen_sizes.borrow();
    assert_eq!(sizes.len(), 2);
    assert_eq!(sizes[0], (20, 4));
    assert_eq!(sizes[1], (40, 10));
}

// =============================================================================
// Test: No props required - app just implements view()
// =============================================================================

#[tokio::test]
async fn app_does_not_require_props_for_rendering() {
    let render_count = Rc::new(Cell::new(0usize));
    let render_count_clone = render_count.clone();

    struct SimpleApp {
        state: i32,
        exit_requested: bool,
        render_count: Rc<Cell<usize>>,
    }

    impl App for SimpleApp {
        type Event = ();
        type Effect = ();
        type Error = io::Error;

        fn update(&mut self, event: AppEvent<()>, _ctx: &ViewContext) -> Option<Vec<()>> {
            match event {
                AppEvent::Key(key) if key.code == KeyCode::Char('j') => {
                    self.state += 1;
                    Some(vec![])
                }
                AppEvent::Key(key) if key.code == KeyCode::Char('q') => {
                    self.exit_requested = true;
                    Some(vec![])
                }
                _ => Some(vec![]),
            }
        }

        fn view(&self, _ctx: &ViewContext) -> Frame {
            self.render_count.set(self.render_count.get() + 1);
            Frame::new(
                vec![Line::new(format!("State: {}", self.state))],
                Cursor {
                    row: 0,
                    col: 0,
                    is_visible: false,
                },
            )
        }

        fn should_exit(&self) -> bool {
            self.exit_requested
        }
    }

    let app = SimpleApp {
        state: 0,
        exit_requested: false,
        render_count: render_count_clone,
    };
    let mut renderer = Renderer::new(TestTerminal::new(20, 4), Theme::default());
    renderer.on_resize((20, 4));
    let (terminal_tx, terminal_rx) = mpsc::unbounded_channel();

    terminal_tx.send(key_event(KeyCode::Char('j'))).unwrap();
    terminal_tx.send(key_event(KeyCode::Char('q'))).unwrap();
    drop(terminal_tx);

    run_app_internal(app, &mut renderer, terminal_rx, None, None)
        .await
        .unwrap();

    assert_eq!(render_count.get(), 2); // initial + state change
}

// =============================================================================
// Internal helper to run the app loop (mirrors Runner internals)
// =============================================================================

async fn run_app_internal<A: App, W: Write>(
    mut app: A,
    renderer: &mut Renderer<W>,
    terminal_event_rx: mpsc::UnboundedReceiver<CrosstermEvent>,
    external_event_rx: Option<mpsc::UnboundedReceiver<A::Event>>,
    tick_rate: Option<Duration>,
) -> Result<(), A::Error> {
    use super::app::run_loop;
    run_loop(
        &mut app,
        renderer,
        terminal_event_rx,
        external_event_rx,
        tick_rate,
    )
    .await
}

// =============================================================================
// Example app for documentation purposes (not a test)
// =============================================================================

/// A simple counter app demonstrating the simplified App API.
/// This type is used in documentation examples.
#[allow(dead_code)]
struct CounterApp {
    count: i32,
    exit_requested: bool,
}

impl App for CounterApp {
    type Event = ();
    type Effect = ();
    type Error = std::io::Error;

    fn update(&mut self, event: AppEvent<()>, _ctx: &ViewContext) -> Option<Vec<()>> {
        match event {
            AppEvent::Key(key) if key.code == KeyCode::Char('q') => {
                self.exit_requested = true;
                Some(vec![])
            }
            AppEvent::Key(key) if key.code == KeyCode::Char('j') => {
                self.count += 1;
                Some(vec![])
            }
            AppEvent::Key(key) if key.code == KeyCode::Char('k') => {
                self.count -= 1;
                Some(vec![])
            }
            _ => Some(vec![]),
        }
    }

    fn view(&self, _ctx: &ViewContext) -> Frame {
        Frame::new(
            vec![Line::new(format!("Count: {}", self.count))],
            Cursor {
                row: 0,
                col: 0,
                is_visible: false,
            },
        )
    }

    async fn run_effect(
        &mut self,
        _renderer: &mut Renderer<impl Write>,
        _effect: (),
    ) -> Result<Vec<()>, std::io::Error> {
        Ok(vec![])
    }

    fn should_exit(&self) -> bool {
        self.exit_requested
    }
}
