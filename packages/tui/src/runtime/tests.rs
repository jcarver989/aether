use super::*;
use crate::component::Cursor;
use crate::rendering::frame::Frame;
use crate::testing::TestTerminal;
use crate::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use crossterm::event::Event as CrosstermEvent;
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

type TerminalEventHandler = Box<
    dyn FnMut(TerminalEvent, &RenderContext, &Rc<RefCell<FakeState>>) -> Vec<Action<FakeEffect>>,
>;
type TickHandler =
    Box<dyn FnMut(&RenderContext, &Rc<RefCell<FakeState>>) -> Vec<Action<FakeEffect>>>;
type CustomEventHandler<E> =
    Box<dyn FnMut(E, &RenderContext, &Rc<RefCell<FakeState>>) -> Vec<Action<FakeEffect>>>;
type EffectHandler = Box<
    dyn FnMut(FakeEffect, &Rc<RefCell<FakeState>>) -> Result<Vec<Action<FakeEffect>>, io::Error>,
>;

struct FakeApp<E> {
    state: Rc<RefCell<FakeState>>,
    on_terminal_event_handler: TerminalEventHandler,
    on_tick_handler: TickHandler,
    on_event_handler: CustomEventHandler<E>,
    on_effect_handler: EffectHandler,
}

impl<E> FakeApp<E> {
    fn new(
        on_terminal_event: impl FnMut(
            TerminalEvent,
            &RenderContext,
            &Rc<RefCell<FakeState>>,
        ) -> Vec<Action<FakeEffect>>
        + 'static,
        on_effect: impl FnMut(
            FakeEffect,
            &Rc<RefCell<FakeState>>,
        ) -> Result<Vec<Action<FakeEffect>>, io::Error>
        + 'static,
    ) -> (Self, Rc<RefCell<FakeState>>) {
        let state = Rc::new(RefCell::new(FakeState::default()));
        (
            Self {
                state: state.clone(),
                on_terminal_event_handler: Box::new(on_terminal_event),
                on_tick_handler: Box::new(|_, _| vec![]),
                on_event_handler: Box::new(|_, _, _| vec![]),
                on_effect_handler: Box::new(on_effect),
            },
            state,
        )
    }

    fn with_tick_handler(
        mut self,
        handler: impl FnMut(&RenderContext, &Rc<RefCell<FakeState>>) -> Vec<Action<FakeEffect>>
        + 'static,
    ) -> Self {
        self.on_tick_handler = Box::new(handler);
        self
    }

    fn with_event_handler(
        mut self,
        handler: impl FnMut(E, &RenderContext, &Rc<RefCell<FakeState>>) -> Vec<Action<FakeEffect>>
        + 'static,
    ) -> Self {
        self.on_event_handler = Box::new(handler);
        self
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

impl<E> App for FakeApp<E> {
    type Event = E;
    type Action = FakeEffect;
    type Error = io::Error;

    fn on_terminal_event(
        &mut self,
        event: TerminalEvent,
        context: &RenderContext,
    ) -> Vec<Action<Self::Action>> {
        (self.on_terminal_event_handler)(event, context, &self.state)
    }

    fn on_tick(&mut self, context: &RenderContext) -> Vec<Action<Self::Action>> {
        (self.on_tick_handler)(context, &self.state)
    }

    fn on_event(
        &mut self,
        event: Self::Event,
        context: &RenderContext,
    ) -> Vec<Action<Self::Action>> {
        (self.on_event_handler)(event, context, &self.state)
    }

    async fn on_action<W: Write>(
        &mut self,
        _renderer: &mut Renderer<W>,
        effect: Self::Action,
    ) -> Result<Vec<Action<Self::Action>>, Self::Error> {
        (self.on_effect_handler)(effect, &self.state)
    }
}

fn key_event(kind: KeyEventKind) -> CrosstermEvent {
    CrosstermEvent::Key(KeyEvent {
        code: KeyCode::Enter,
        modifiers: KeyModifiers::NONE,
        kind,
        state: crate::KeyEventState::NONE,
    })
}

#[tokio::test]
async fn initial_render_happens_before_event_loop_work() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let log_clone = log.clone();
    let (mut app, state) = FakeApp::new(
        move |_, _, _| {
            log_clone.borrow_mut().push("event");
            vec![Action::Exit]
        },
        |_, _| Ok(vec![]),
    );
    let mut renderer = Renderer::new(TestTerminal::new(20, 4), Theme::default());
    renderer.on_resize((20, 4));
    let (terminal_tx, terminal_rx) = mpsc::unbounded_channel();
    terminal_tx.send(key_event(KeyEventKind::Press)).unwrap();
    drop(terminal_tx);

    run_app(
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
async fn resize_is_runtime_managed_and_triggers_rerender() {
    let terminal_events = Rc::new(RefCell::new(Vec::new()));
    let terminal_events_clone = terminal_events.clone();
    let (mut app, state) = FakeApp::new(
        move |event, _, _| {
            terminal_events_clone.borrow_mut().push(match event {
                TerminalEvent::Key(_) => "key",
                TerminalEvent::Paste(_) => "paste",
            });
            vec![Action::Exit]
        },
        |_, _| Ok(vec![]),
    );
    let mut renderer = Renderer::new(TestTerminal::new(10, 3), Theme::default());
    renderer.on_resize((10, 3));
    let (terminal_tx, terminal_rx) = mpsc::unbounded_channel();
    terminal_tx.send(CrosstermEvent::Resize(42, 12)).unwrap();
    terminal_tx.send(key_event(KeyEventKind::Press)).unwrap();
    drop(terminal_tx);

    run_app(
        &mut app,
        &mut renderer,
        terminal_rx,
        None::<mpsc::UnboundedReceiver<()>>,
        None,
    )
    .await
    .unwrap();

    assert_eq!(terminal_events.borrow().as_slice(), &["key"]);
    assert_eq!(state.borrow().renders.as_slice(), &[(10, 3), (42, 12)]);
}

#[tokio::test]
async fn render_actions_coalesce_into_one_render_per_batch() {
    let (mut app, state) = FakeApp::new(
        |_, _, _| vec![Action::Render, Action::Render, Action::Exit],
        |_, _| Ok(vec![]),
    );
    let mut renderer = Renderer::new(TestTerminal::new(20, 4), Theme::default());
    renderer.on_resize((20, 4));
    let (terminal_tx, terminal_rx) = mpsc::unbounded_channel();
    terminal_tx.send(key_event(KeyEventKind::Press)).unwrap();
    drop(terminal_tx);

    run_app(
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
    let log: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
    let event_log = log.clone();
    let effect_log = log.clone();
    let (mut app, state) = FakeApp::new(
        move |_, _, _| {
            event_log.borrow_mut().push("event".to_string());
            vec![Action::Custom(FakeEffect::FollowUp)]
        },
        move |effect, _| match effect {
            FakeEffect::FollowUp => {
                effect_log.borrow_mut().push("effect:follow-up".to_string());
                Ok(vec![
                    Action::Render,
                    Action::Custom(FakeEffect::Log("after-render")),
                    Action::Exit,
                ])
            }
            FakeEffect::Log(message) => {
                effect_log.borrow_mut().push(message.to_string());
                Ok(vec![])
            }
        },
    );
    let mut renderer = Renderer::new(TestTerminal::new(20, 4), Theme::default());
    renderer.on_resize((20, 4));
    let (terminal_tx, terminal_rx) = mpsc::unbounded_channel();
    terminal_tx.send(key_event(KeyEventKind::Press)).unwrap();
    drop(terminal_tx);

    run_app(
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
        &[
            "event".to_string(),
            "effect:follow-up".to_string(),
            "after-render".to_string()
        ]
    );
    assert_eq!(state.borrow().renders.len(), 2);
}

#[tokio::test]
async fn exit_action_stops_the_loop_cleanly() {
    let (mut app, _) = FakeApp::new(|_, _, _| vec![Action::Exit], |_, _| Ok(vec![]));
    let mut renderer = Renderer::new(TestTerminal::new(20, 4), Theme::default());
    renderer.on_resize((20, 4));
    let (terminal_tx, terminal_rx) = mpsc::unbounded_channel();
    terminal_tx.send(key_event(KeyEventKind::Press)).unwrap();
    drop(terminal_tx);

    let result = run_app(
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
    let (app, _) = FakeApp::new(|_, _, _| vec![Action::Exit], |_, _| Ok(vec![]));
    let mut app = app.with_tick_handler(move |_, _| {
        *saw_tick_clone.borrow_mut() = true;
        vec![]
    });
    let mut renderer = Renderer::new(TestTerminal::new(20, 4), Theme::default());
    renderer.on_resize((20, 4));
    let (terminal_tx, terminal_rx) = mpsc::unbounded_channel();
    terminal_tx.send(key_event(KeyEventKind::Press)).unwrap();
    drop(terminal_tx);

    run_app(
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
    let terminal_events = events.clone();
    let (app, _) = FakeApp::new(
        move |_, _, _| {
            terminal_events.borrow_mut().push("terminal");
            vec![Action::Exit]
        },
        |_, _| Ok(vec![]),
    );
    let external_events = events.clone();
    let mut app = app.with_event_handler(move |_, _, _| {
        external_events.borrow_mut().push("external");
        vec![Action::Exit]
    });
    let mut renderer = Renderer::new(TestTerminal::new(20, 4), Theme::default());
    renderer.on_resize((20, 4));
    let (terminal_tx, terminal_rx) = mpsc::unbounded_channel();
    terminal_tx.send(key_event(KeyEventKind::Press)).unwrap();
    drop(terminal_tx);

    run_app(
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
        events: Vec<String>,
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

    impl App for TraitApp {
        type Event = ();
        type Action = FakeEffect;
        type Error = io::Error;

        fn on_terminal_event(
            &mut self,
            _event: TerminalEvent,
            _context: &RenderContext,
        ) -> Vec<Action<Self::Action>> {
            self.events.push("event".to_string());
            vec![Action::Custom(FakeEffect::Log("from-event"))]
        }

        fn on_event(
            &mut self,
            _event: Self::Event,
            _context: &RenderContext,
        ) -> Vec<Action<Self::Action>> {
            vec![]
        }

        async fn on_action<W: Write>(
            &mut self,
            _renderer: &mut Renderer<W>,
            effect: Self::Action,
        ) -> Result<Vec<Action<Self::Action>>, Self::Error> {
            match effect {
                FakeEffect::Log(message) => {
                    self.events.push(message.to_string());
                    Ok(vec![Action::Exit])
                }
                FakeEffect::FollowUp => Ok(vec![Action::Exit]),
            }
        }
    }

    let mut app = TraitApp {
        renders: 0,
        events: Vec::new(),
    };
    let mut renderer = Renderer::new(TestTerminal::new(20, 4), Theme::default());
    renderer.on_resize((20, 4));
    let (terminal_tx, terminal_rx) = mpsc::unbounded_channel();
    terminal_tx.send(key_event(KeyEventKind::Press)).unwrap();
    drop(terminal_tx);

    run_app(
        &mut app,
        &mut renderer,
        terminal_rx,
        None::<mpsc::UnboundedReceiver<()>>,
        None,
    )
    .await
    .unwrap();

    assert_eq!(app.events, vec!["event", "from-event"]);
    assert_eq!(app.renders, 1);
}

#[tokio::test]
async fn effect_follow_up_render_happens_before_follow_up_effect() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let effect_log = log.clone();
    let (mut app, state) = FakeApp::new(
        |_, _, _| vec![Action::Custom(FakeEffect::FollowUp)],
        move |effect, state| match effect {
            FakeEffect::FollowUp => Ok(vec![
                Action::Render,
                Action::Custom(FakeEffect::Log("after-render")),
                Action::Exit,
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
    renderer.on_resize((20, 4));
    let (terminal_tx, terminal_rx) = mpsc::unbounded_channel();
    terminal_tx.send(key_event(KeyEventKind::Press)).unwrap();
    drop(terminal_tx);

    run_app(
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

#[tokio::test]
async fn on_event_can_return_only_render() {
    let (mut app, state) = FakeApp::new(|_, _, _| vec![Action::Render], |_, _| Ok(vec![]));
    let mut renderer = Renderer::new(TestTerminal::new(20, 4), Theme::default());
    renderer.on_resize((20, 4));
    let (terminal_tx, terminal_rx) = mpsc::unbounded_channel();
    terminal_tx.send(key_event(KeyEventKind::Press)).unwrap();
    drop(terminal_tx);

    run_app(
        &mut app,
        &mut renderer,
        terminal_rx,
        None::<mpsc::UnboundedReceiver<()>>,
        None,
    )
    .await
    .unwrap();

    // Initial render + one render from on_event
    assert_eq!(state.borrow().renders.len(), 2);
}

#[tokio::test]
async fn multiple_render_actions_coalesce() {
    let (mut app, state) = FakeApp::new(
        |_, _, _| vec![Action::Render, Action::Render],
        |_, _| Ok(vec![]),
    );
    let mut renderer = Renderer::new(TestTerminal::new(20, 4), Theme::default());
    renderer.on_resize((20, 4));
    let (terminal_tx, terminal_rx) = mpsc::unbounded_channel();
    terminal_tx.send(key_event(KeyEventKind::Press)).unwrap();
    drop(terminal_tx);

    run_app(
        &mut app,
        &mut renderer,
        terminal_rx,
        None::<mpsc::UnboundedReceiver<()>>,
        None,
    )
    .await
    .unwrap();

    // Initial render + one coalesced render
    assert_eq!(state.borrow().renders.len(), 2);
}

#[tokio::test]
async fn on_event_receives_current_render_context() {
    let seen_size = Rc::new(RefCell::new(None::<(u16, u16)>));
    let seen_size_clone = seen_size.clone();
    let (mut app, _) = FakeApp::new(
        move |_, context, _| {
            *seen_size_clone.borrow_mut() = Some((context.size.width, context.size.height));
            vec![Action::Exit]
        },
        |_, _| Ok(vec![]),
    );
    let mut renderer = Renderer::new(TestTerminal::new(20, 4), Theme::default());
    renderer.on_resize((42, 12));
    let (terminal_tx, terminal_rx) = mpsc::unbounded_channel();
    terminal_tx.send(key_event(KeyEventKind::Press)).unwrap();
    drop(terminal_tx);

    run_app(
        &mut app,
        &mut renderer,
        terminal_rx,
        None::<mpsc::UnboundedReceiver<()>>,
        None,
    )
    .await
    .unwrap();

    assert_eq!(*seen_size.borrow(), Some((42, 12)));
}
