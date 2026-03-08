use super::event_loop::run_event_loop;
use super::*;
use crate::component::Cursor;
use crate::rendering::frame::Frame;
use crate::testing::TestTerminal;
use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind};
use std::cell::RefCell;
use std::io::{self, Write};
use std::rc::Rc;
use tokio::sync::mpsc;

#[derive(Default)]
struct FakeState {
    renders: Vec<(u16, u16)>,
}

enum FakeEffect {
    Log(&'static str),
    FollowUp,
}

type EventHandler<E> = Box<
    dyn FnMut(
        RuntimeEvent<E>,
        &RenderContext,
        &Rc<RefCell<FakeState>>,
    ) -> Vec<RuntimeAction<FakeEffect>>,
>;
type EffectHandler = Box<
    dyn FnMut(
        FakeEffect,
        &Rc<RefCell<FakeState>>,
    ) -> Result<Vec<RuntimeAction<FakeEffect>>, io::Error>,
>;

struct FakeApp<E> {
    state: Rc<RefCell<FakeState>>,
    on_event: EventHandler<E>,
    on_effect: EffectHandler,
}

impl<E> FakeApp<E> {
    fn new(
        on_event: impl FnMut(
            RuntimeEvent<E>,
            &RenderContext,
            &Rc<RefCell<FakeState>>,
        ) -> Vec<RuntimeAction<FakeEffect>>
        + 'static,
        on_effect: impl FnMut(
            FakeEffect,
            &Rc<RefCell<FakeState>>,
        ) -> Result<Vec<RuntimeAction<FakeEffect>>, io::Error>
        + 'static,
    ) -> (Self, Rc<RefCell<FakeState>>) {
        let state = Rc::new(RefCell::new(FakeState::default()));
        (
            Self {
                state: state.clone(),
                on_event: Box::new(on_event),
                on_effect: Box::new(on_effect),
            },
            state,
        )
    }
}

impl<E> RootComponent for FakeApp<E> {
    fn render(&mut self, context: &RenderContext) -> Frame {
        self.state
            .borrow_mut()
            .renders
            .push((context.size.width, context.size.height));
        Frame::new(
            vec![crate::Line::new("frame")],
            Cursor {
                row: 0,
                col: 0,
                is_visible: false,
            },
        )
    }
}

impl<E> RuntimeApp for FakeApp<E> {
    type External = E;
    type Effect = FakeEffect;
    type Error = io::Error;

    fn on_event(
        &mut self,
        event: RuntimeEvent<Self::External>,
        context: &RenderContext,
    ) -> Vec<RuntimeAction<Self::Effect>> {
        (self.on_event)(event, context, &self.state)
    }

    async fn on_effect<W: Write>(
        &mut self,
        _renderer: &mut Renderer<W>,
        effect: Self::Effect,
    ) -> Result<Vec<RuntimeAction<Self::Effect>>, Self::Error> {
        (self.on_effect)(effect, &self.state)
    }
}

fn key_event(kind: KeyEventKind) -> Event {
    Event::Key(KeyEvent {
        code: KeyCode::Enter,
        modifiers: crossterm::event::KeyModifiers::NONE,
        kind,
        state: crossterm::event::KeyEventState::NONE,
    })
}

#[tokio::test]
async fn initial_render_happens_before_event_loop_work() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let log_clone = log.clone();
    let (mut app, state) = FakeApp::new(
        move |_, _, _| {
            log_clone.borrow_mut().push("event");
            vec![RuntimeAction::Exit]
        },
        |_, _| Ok(vec![]),
    );
    let mut renderer = Renderer::new(TestTerminal::new(20, 4), Theme::default());
    renderer.update_render_context_with((20, 4));
    let (terminal_tx, terminal_rx) = mpsc::unbounded_channel();
    terminal_tx.send(key_event(KeyEventKind::Press)).unwrap();
    drop(terminal_tx);

    run_event_loop(
        &mut app,
        &mut renderer,
        terminal_rx,
        None::<mpsc::UnboundedReceiver<()>>,
        None,
    )
    .await
    .unwrap();

    assert_eq!(state.borrow().renders.len(), 1);
    assert_eq!(log.borrow().as_slice(), &["event"]);
}

#[tokio::test]
async fn resize_updates_render_context_before_event_handling() {
    let seen_sizes = Rc::new(RefCell::new(Vec::new()));
    let seen_sizes_clone = seen_sizes.clone();
    let (mut app, _) = FakeApp::new(
        move |event, context, _| {
            if matches!(event, RuntimeEvent::Terminal(Event::Resize(..))) {
                seen_sizes_clone
                    .borrow_mut()
                    .push((context.size.width, context.size.height));
            }
            vec![RuntimeAction::Exit]
        },
        |_, _| Ok(vec![]),
    );
    let mut renderer = Renderer::new(TestTerminal::new(10, 3), Theme::default());
    renderer.update_render_context_with((10, 3));
    let (terminal_tx, terminal_rx) = mpsc::unbounded_channel();
    terminal_tx.send(Event::Resize(42, 12)).unwrap();
    drop(terminal_tx);

    run_event_loop(
        &mut app,
        &mut renderer,
        terminal_rx,
        None::<mpsc::UnboundedReceiver<()>>,
        None,
    )
    .await
    .unwrap();

    assert_eq!(seen_sizes.borrow().as_slice(), &[(42, 12)]);
}

#[tokio::test]
async fn render_actions_coalesce_into_one_render_per_batch() {
    let (mut app, state) = FakeApp::new(
        |_, _, _| {
            vec![
                RuntimeAction::Render,
                RuntimeAction::Render,
                RuntimeAction::Exit,
            ]
        },
        |_, _| Ok(vec![]),
    );
    let mut renderer = Renderer::new(TestTerminal::new(20, 4), Theme::default());
    renderer.update_render_context_with((20, 4));
    let (terminal_tx, terminal_rx) = mpsc::unbounded_channel();
    terminal_tx.send(key_event(KeyEventKind::Press)).unwrap();
    drop(terminal_tx);

    run_event_loop(
        &mut app,
        &mut renderer,
        terminal_rx,
        None::<mpsc::UnboundedReceiver<()>>,
        None,
    )
    .await
    .unwrap();

    assert_eq!(state.borrow().renders.len(), 1);
}

#[tokio::test]
async fn effect_follow_up_actions_are_processed_in_order() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let event_log = log.clone();
    let effect_log = log.clone();
    let (mut app, state) = FakeApp::new(
        move |_, _, _| {
            event_log.borrow_mut().push("event");
            vec![RuntimeAction::Effect(FakeEffect::FollowUp)]
        },
        move |effect, _| match effect {
            FakeEffect::FollowUp => {
                effect_log.borrow_mut().push("effect:follow-up");
                Ok(vec![
                    RuntimeAction::Render,
                    RuntimeAction::Effect(FakeEffect::Log("after-render")),
                    RuntimeAction::Exit,
                ])
            }
            FakeEffect::Log(message) => {
                effect_log.borrow_mut().push(message);
                Ok(vec![])
            }
        },
    );
    let mut renderer = Renderer::new(TestTerminal::new(20, 4), Theme::default());
    renderer.update_render_context_with((20, 4));
    let (terminal_tx, terminal_rx) = mpsc::unbounded_channel();
    terminal_tx.send(key_event(KeyEventKind::Press)).unwrap();
    drop(terminal_tx);

    run_event_loop(
        &mut app,
        &mut renderer,
        terminal_rx,
        None::<mpsc::UnboundedReceiver<()>>,
        None,
    )
    .await
    .unwrap();

    assert_eq!(
        log.borrow().as_slice(),
        &["event", "effect:follow-up", "after-render"]
    );
    assert_eq!(state.borrow().renders.len(), 2);
}

#[tokio::test]
async fn exit_action_stops_the_loop_cleanly() {
    let (mut app, _) = FakeApp::new(|_, _, _| vec![RuntimeAction::Exit], |_, _| Ok(vec![]));
    let mut renderer = Renderer::new(TestTerminal::new(20, 4), Theme::default());
    renderer.update_render_context_with((20, 4));
    let (terminal_tx, terminal_rx) = mpsc::unbounded_channel();
    terminal_tx.send(key_event(KeyEventKind::Press)).unwrap();
    drop(terminal_tx);

    let result = run_event_loop(
        &mut app,
        &mut renderer,
        terminal_rx,
        None::<mpsc::UnboundedReceiver<()>>,
        None,
    )
    .await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn tick_source_is_optional() {
    let saw_tick = Rc::new(RefCell::new(false));
    let saw_tick_clone = saw_tick.clone();
    let (mut app, _) = FakeApp::new(
        move |event, _, _| {
            if matches!(event, RuntimeEvent::Tick(_)) {
                *saw_tick_clone.borrow_mut() = true;
            }
            vec![RuntimeAction::Exit]
        },
        |_, _| Ok(vec![]),
    );
    let mut renderer = Renderer::new(TestTerminal::new(20, 4), Theme::default());
    renderer.update_render_context_with((20, 4));
    let (terminal_tx, terminal_rx) = mpsc::unbounded_channel();
    terminal_tx.send(key_event(KeyEventKind::Press)).unwrap();
    drop(terminal_tx);

    run_event_loop(
        &mut app,
        &mut renderer,
        terminal_rx,
        None::<mpsc::UnboundedReceiver<()>>,
        None,
    )
    .await
    .unwrap();

    assert!(!*saw_tick.borrow());
}

#[tokio::test]
async fn external_event_source_is_optional() {
    let events = Rc::new(RefCell::new(Vec::new()));
    let events_clone = events.clone();
    let (mut app, _) = FakeApp::new(
        move |event, _, _| {
            events_clone.borrow_mut().push(match event {
                RuntimeEvent::Terminal(_) => "terminal",
                RuntimeEvent::Tick(_) => "tick",
                RuntimeEvent::External(_) => "external",
            });
            vec![RuntimeAction::Exit]
        },
        |_, _| Ok(vec![]),
    );
    let mut renderer = Renderer::new(TestTerminal::new(20, 4), Theme::default());
    renderer.update_render_context_with((20, 4));
    let (terminal_tx, terminal_rx) = mpsc::unbounded_channel();
    terminal_tx.send(key_event(KeyEventKind::Press)).unwrap();
    drop(terminal_tx);

    run_event_loop(
        &mut app,
        &mut renderer,
        terminal_rx,
        None::<mpsc::UnboundedReceiver<&'static str>>,
        None,
    )
    .await
    .unwrap();

    assert_eq!(events.borrow().as_slice(), &["terminal"]);
}

#[tokio::test]
async fn run_app_uses_trait_based_handlers() {
    struct TraitApp {
        renders: usize,
        events: Vec<&'static str>,
    }

    impl RootComponent for TraitApp {
        fn render(&mut self, _context: &RenderContext) -> Frame {
            self.renders += 1;
            Frame::new(
                vec![crate::Line::new("trait")],
                Cursor {
                    row: 0,
                    col: 0,
                    is_visible: false,
                },
            )
        }
    }

    impl RuntimeApp for TraitApp {
        type External = ();
        type Effect = FakeEffect;
        type Error = io::Error;

        fn on_event(
            &mut self,
            _event: RuntimeEvent<Self::External>,
            _context: &RenderContext,
        ) -> Vec<RuntimeAction<Self::Effect>> {
            self.events.push("event");
            vec![RuntimeAction::Effect(FakeEffect::Log("effect"))]
        }

        async fn on_effect<W: Write>(
            &mut self,
            _renderer: &mut Renderer<W>,
            effect: Self::Effect,
        ) -> Result<Vec<RuntimeAction<Self::Effect>>, Self::Error> {
            match effect {
                FakeEffect::Log(message) => {
                    self.events.push(message);
                    Ok(vec![RuntimeAction::Exit])
                }
                FakeEffect::FollowUp => Ok(vec![RuntimeAction::Exit]),
            }
        }
    }

    let mut app = TraitApp {
        renders: 0,
        events: Vec::new(),
    };
    let mut renderer = Renderer::new(TestTerminal::new(20, 4), Theme::default());
    renderer.update_render_context_with((20, 4));
    let (terminal_tx, terminal_rx) = mpsc::unbounded_channel();
    terminal_tx.send(key_event(KeyEventKind::Press)).unwrap();
    drop(terminal_tx);

    run_event_loop(
        &mut app,
        &mut renderer,
        terminal_rx,
        None::<mpsc::UnboundedReceiver<()>>,
        None,
    )
    .await
    .unwrap();

    assert_eq!(app.events, vec!["event", "effect"]);
    assert_eq!(app.renders, 1);
}

#[tokio::test]
async fn effect_follow_up_render_happens_before_follow_up_effect() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let effect_log = log.clone();
    let (mut app, state) = FakeApp::new(
        |_, _, _| vec![RuntimeAction::Effect(FakeEffect::FollowUp)],
        move |effect, state| match effect {
            FakeEffect::FollowUp => Ok(vec![
                RuntimeAction::Render,
                RuntimeAction::Effect(FakeEffect::Log("after-render")),
                RuntimeAction::Exit,
            ]),
            FakeEffect::Log(message) => {
                effect_log.borrow_mut().push(message);
                effect_log
                    .borrow_mut()
                    .push(if state.borrow().renders.len() == 2 {
                        "rendered"
                    } else {
                        "not-rendered"
                    });
                Ok(vec![])
            }
        },
    );
    let mut renderer = Renderer::new(TestTerminal::new(20, 4), Theme::default());
    renderer.update_render_context_with((20, 4));
    let (terminal_tx, terminal_rx) = mpsc::unbounded_channel();
    terminal_tx.send(key_event(KeyEventKind::Press)).unwrap();
    drop(terminal_tx);

    run_event_loop(
        &mut app,
        &mut renderer,
        terminal_rx,
        None::<mpsc::UnboundedReceiver<()>>,
        None,
    )
    .await
    .unwrap();

    assert_eq!(log.borrow().as_slice(), &["after-render", "rendered"]);
    assert_eq!(state.borrow().renders.len(), 2);
}
