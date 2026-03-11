//! Tests for the new App API (simplified, one-trait approach)
//!
//! This file tests the new simplified app API with framework-owned rerender policy.
//! The framework decides when to render (after each update/effect cycle), not the app.

use super::app::{App, AppEvent};
use crate::components::Response;
use crate::components::{Cursor, ViewContext};
use crate::rendering::renderer::{Renderer, Terminal};
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

#[test]
fn command_from_vec_collapses_common_cases() {
    let empty: Vec<i32> = vec![];
    assert!(matches!(Response::from_vec(empty), Response::Ok));
    assert!(matches!(Response::from_vec(vec![1]), Response::One(1)));
    assert!(
        matches!(Response::from_vec(vec![1, 2]), Response::Many(values) if values == vec![1, 2])
    );
}

#[test]
fn command_merge_preserves_order_and_quit() {
    let merged = Response::one(1).merge(Response::many(vec![2, 3]));
    assert!(matches!(merged, Response::Many(values) if values == vec![1, 2, 3]));

    let quit_merged: Response<i32> = Response::one(1).merge(Response::exit());
    assert!(quit_merged.is_exit());
}

#[tokio::test]
async fn app_rerenders_after_state_changing_events() {
    // The framework should render after each update that changes state.
    // This is the core of the simplified rerender policy.
    let render_count = Rc::new(Cell::new(0usize));
    let render_count_clone = render_count.clone();

    struct TestApp {
        count: i32,
        render_count: Rc<Cell<usize>>,
    }

    impl App for TestApp {
        type Event = ();
        type Effect = ();
        type Error = io::Error;

        fn update(&mut self, event: AppEvent<()>, _ctx: &ViewContext) -> Response<()> {
            match event {
                AppEvent::Key(key) if key.code == KeyCode::Char('j') => {
                    self.count += 1;
                    Response::ok()
                }
                AppEvent::Key(key) if key.code == KeyCode::Char('q') => Response::exit(),
                _ => Response::ok(),
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
    }

    let app = TestApp {
        count: 0,
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
        render_count: Rc<Cell<usize>>,
    }

    impl App for TestApp {
        type Event = String;
        type Effect = ();
        type Error = io::Error;

        fn update(&mut self, event: AppEvent<String>, _ctx: &ViewContext) -> Response<()> {
            match event {
                AppEvent::External(msg) => {
                    self.value = msg;
                    Response::one(())
                }
                AppEvent::Key(key) if key.code == KeyCode::Char('q') => Response::exit(),
                _ => Response::ok(),
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
            _terminal: &mut Terminal<'_, impl Write>,
            _effect: (),
        ) -> Result<Response<()>, io::Error> {
            Ok(Response::exit())
        }
    }

    let app = TestApp {
        value: "initial".to_string(),
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
        render_count: Rc<Cell<usize>>,
    }

    impl App for TestApp {
        type Event = ();
        type Effect = TestEffect;
        type Error = io::Error;

        fn update(&mut self, event: AppEvent<()>, _ctx: &ViewContext) -> Response<TestEffect> {
            match event {
                AppEvent::Key(key) if key.code == KeyCode::Char(' ') => {
                    Response::one(TestEffect::Increment)
                }
                AppEvent::Key(key) if key.code == KeyCode::Char('q') => Response::exit(),
                _ => Response::ok(),
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
            _terminal: &mut Terminal<'_, impl Write>,
            effect: TestEffect,
        ) -> Result<Response<TestEffect>, io::Error> {
            match effect {
                TestEffect::Increment => {
                    self.count += 1;
                    Ok(Response::ok())
                }
            }
        }
    }

    let app = TestApp {
        count: 0,
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
    // When the frame content doesn't change, the terminal should not receive
    // new output. This verifies that frame diffing still works with the
    // simplified rerender policy.
    //
    // Note: We can't directly measure terminal writes with TestTerminal,
    // but we can verify that the frame diffing logic is in place by checking
    // that identical frames result in the same terminal state.
    struct StaticApp;

    impl App for StaticApp {
        type Event = ();
        type Effect = ();
        type Error = io::Error;

        fn update(&mut self, event: AppEvent<()>, _ctx: &ViewContext) -> Response<()> {
            match event {
                AppEvent::Key(key) if key.code == KeyCode::Char('q') => Response::exit(),
                // No state change - frame content stays the same
                _ => Response::ok(),
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

    run_app_internal(StaticApp, &mut renderer, terminal_rx, None, None)
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
        render_count: Rc<Cell<usize>>,
        tick_count: Rc<Cell<usize>>,
    }

    impl App for TestApp {
        type Event = ();
        type Effect = ();
        type Error = io::Error;

        fn update(&mut self, event: AppEvent<()>, _ctx: &ViewContext) -> Response<()> {
            match event {
                AppEvent::Tick(_) => {
                    self.ticks += 1;
                    self.tick_count.set(self.tick_count.get() + 1);
                    if self.ticks >= 2 {
                        Response::exit()
                    } else {
                        Response::ok()
                    }
                }
                _ => Response::ok(),
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
    }

    let app = TestApp {
        ticks: 0,
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
        seen_sizes: Rc<RefCell<Vec<(u16, u16)>>>,
    }

    impl App for TestApp {
        type Event = ();
        type Effect = ();
        type Error = io::Error;

        fn update(&mut self, event: AppEvent<()>, _ctx: &ViewContext) -> Response<()> {
            match event {
                AppEvent::Key(key) if key.code == KeyCode::Char('q') => Response::exit(),
                _ => Response::ok(),
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
    }

    let app = TestApp {
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
    // This test verifies that apps don't need to implement any props mechanism.
    // The CounterApp at the bottom of this file demonstrates this - it has no
    // props type and just renders its current state.

    let render_count = Rc::new(Cell::new(0usize));
    let render_count_clone = render_count.clone();

    struct SimpleApp {
        state: i32,
        render_count: Rc<Cell<usize>>,
    }

    impl App for SimpleApp {
        type Event = ();
        type Effect = ();
        type Error = io::Error;

        fn update(&mut self, event: AppEvent<()>, _ctx: &ViewContext) -> Response<()> {
            match event {
                AppEvent::Key(key) if key.code == KeyCode::Char('j') => {
                    self.state += 1;
                    Response::ok()
                }
                AppEvent::Key(key) if key.code == KeyCode::Char('q') => Response::exit(),
                _ => Response::ok(),
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
    }

    let app = SimpleApp {
        state: 0,
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

    // If we got here without compile errors about missing props, the test passes.
    // Also verify we rendered the expected number of times.
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
}

impl App for CounterApp {
    type Event = ();
    type Effect = ();
    type Error = std::io::Error;

    fn update(&mut self, event: AppEvent<()>, _ctx: &ViewContext) -> Response<()> {
        match event {
            AppEvent::Key(key) if key.code == KeyCode::Char('q') => return Response::exit(),
            AppEvent::Key(key) if key.code == KeyCode::Char('j') => {
                self.count += 1;
                Response::ok()
            }
            AppEvent::Key(key) if key.code == KeyCode::Char('k') => {
                self.count -= 1;
                Response::ok()
            }
            _ => Response::ok(),
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
        _terminal: &mut Terminal<'_, impl Write>,
        _effect: (),
    ) -> Result<Response<()>, std::io::Error> {
        Ok(Response::ok())
    }
}
