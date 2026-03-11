//! Simplified application API for building full-screen terminal apps.
//!
//! This module provides the primary app-facing API with one unified model:
//!
//! - [`App`] — single trait combining event handling, effects, and rendering
//! - [`AppEvent`] — unified event type for terminal, external, and tick events
//! - [`Response`] — unified result type for event handling and effects
//! - [`Runner`] — builder-style runner that owns terminal lifecycle
//!
//! # Example
//!
//! ```rust
//! use tui::{App, AppEvent, Response, Frame, Line, ViewContext, Runner};
//! use tui::{KeyEvent, KeyCode, KeyModifiers};
//!
//! struct Counter {
//!     count: i32,
//! }
//!
//! impl App for Counter {
//!     type Event = ();
//!     type Effect = ();
//!     type Error = std::io::Error;
//!
//!     fn update(&mut self, event: AppEvent<()>, ctx: &ViewContext) -> Response<()> {
//!         match event {
//!             AppEvent::Key(key) if key.code == KeyCode::Char('q') => Response::exit(),
//!             AppEvent::Key(key) if key.code == KeyCode::Char('j') => {
//!                 self.count += 1;
//!                 Response::ok()
//!             }
//!             AppEvent::Key(key) if key.code == KeyCode::Char('k') => {
//!                 self.count -= 1;
//!                 Response::ok()
//!             }
//!             _ => Response::ok(),
//!         }
//!     }
//!
//!     fn view(&self, ctx: &ViewContext) -> Frame {
//!         Frame::new(
//!             vec![Line::new(format!("Count: {}", self.count))],
//!             tui::Cursor { row: 0, col: 0, is_visible: false },
//!         )
//!     }
//!
//!     async fn run_effect(
//!         &mut self,
//!         _terminal: &mut tui::advanced::Terminal<'_, impl std::io::Write>,
//!         _effect: (),
//!     ) -> Result<Response<()>, std::io::Error> {
//!         Ok(Response::ok())
//!     }
//! }
//! ```

use super::spawn_terminal_event_task;
use super::terminal::{MouseCapture, TerminalSession, terminal_size};
use crate::Frame;
use crate::components::Response;
use crate::rendering::render_context::ViewContext;
use crate::rendering::renderer::{Renderer, Terminal};
use crate::rendering::size::Size;
use crate::theme::Theme;
use crossterm::event::{Event as CrosstermEvent, KeyEvent, KeyEventKind, MouseEvent};
use std::collections::VecDeque;
use std::io::{self, Write};
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tokio::time::{self, Interval};

/// Unified event type for terminal applications.
///
/// Combines terminal events, external events, and tick events into a single enum,
/// allowing apps to handle all event types in one [`App::update`] method.
#[derive(Debug, Clone)]
pub enum AppEvent<E> {
    /// A keyboard event.
    Key(KeyEvent),
    /// Pasted text from bracketed paste mode.
    Paste(String),
    /// A mouse event.
    Mouse(MouseEvent),
    /// A tick event for time-based updates.
    Tick(Instant),
    /// Terminal was resized.
    Resize(Size),
    /// An application-specific external event.
    External(E),
}

/// The primary application trait for building full-screen terminal apps.
///
/// This trait unifies event handling, effects, and rendering into a single coherent model.
/// Apps implement `update` to handle events and `view` to render.
///
/// # Associated Types
///
/// - `Event` — Application-specific external event type
/// - `Effect` — Application-specific effect/command type
/// - `Error` — Error type for effect execution
///
/// # Lifecycle
///
/// 1. `view` is called for the initial render
/// 2. Events arrive via `update`
/// 3. Effects from `update` are processed via `run_effect`
/// 4. After each update/effect cycle, `view` is called again
/// 5. When `update` returns `Response::exit()`, the app terminates
#[allow(async_fn_in_trait)]
pub trait App {
    /// Application-specific external event type.
    type Event;
    /// Application-specific effect/command type.
    type Effect;
    /// Error type for effect execution.
    type Error: From<io::Error>;

    /// Handle an event and return effects.
    ///
    /// This is the main event handler for the application. All terminal events,
    /// external events, and ticks flow through this method.
    fn update(
        &mut self,
        event: AppEvent<Self::Event>,
        ctx: &ViewContext,
    ) -> Response<Self::Effect>;

    /// Render the current application state.
    ///
    /// Called after each update/effect cycle. The framework handles frame diffing
    /// to minimize terminal writes.
    fn view(&self, ctx: &ViewContext) -> Frame;

    /// Execute an effect and return follow-up effects.
    ///
    /// Effects allow async operations like network requests, file I/O, etc.
    /// A [`Terminal`] handle is provided for effects that need terminal
    /// operations (e.g., pushing to scrollback, clearing screen, changing theme).
    ///
    /// Default implementation returns no effects.
    async fn run_effect(
        &mut self,
        _terminal: &mut Terminal<'_, impl Write>,
        effect: Self::Effect,
    ) -> Result<Response<Self::Effect>, Self::Error> {
        // Default: consume the effect without action
        let _ = effect;
        Ok(Response::ok())
    }

    /// Whether the app wants tick events.
    ///
    /// If this returns `false` (the default), tick events are not generated
    /// even if a tick rate is configured.
    fn wants_tick(&self) -> bool {
        false
    }
}

/// Builder-style runner for terminal applications.
///
/// Owns terminal lifecycle, event loop setup, and cleanup.
///
/// # Example
///
/// ```rust,ignore
/// use tui::Runner;
/// use std::time::Duration;
///
/// # async fn example(my_app: impl tui::App) -> Result<(), Box<dyn std::error::Error>> {
/// Runner::new(my_app)
///     .tick_rate(Duration::from_millis(100))
///     .run()
///     .await?;
/// # Ok(())
/// # }
/// ```
pub struct Runner<A: App> {
    app: A,
    theme: Theme,
    tick_rate: Option<Duration>,
    external_events: Option<mpsc::UnboundedReceiver<A::Event>>,
    mouse_capture: MouseCapture,
    enable_bracketed_paste: bool,
}

impl<A: App> Runner<A> {
    /// Create a new runner for the given app.
    pub fn new(app: A) -> Self {
        Self {
            app,
            theme: Theme::default(),
            tick_rate: Some(Duration::from_millis(100)),
            external_events: None,
            mouse_capture: MouseCapture::Disabled,
            enable_bracketed_paste: true,
        }
    }

    /// Set the theme for rendering.
    pub fn theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }

    /// Set the tick rate for tick events.
    ///
    /// Set to `None` to disable ticks entirely.
    pub fn tick_rate(mut self, rate: Duration) -> Self {
        self.tick_rate = Some(rate);
        self
    }

    /// Disable tick events.
    pub fn no_ticks(mut self) -> Self {
        self.tick_rate = None;
        self
    }

    /// Provide an external event channel.
    pub fn external_events(mut self, rx: mpsc::UnboundedReceiver<A::Event>) -> Self {
        self.external_events = Some(rx);
        self
    }

    /// Enable or disable mouse capture.
    pub fn mouse_capture(mut self, capture: MouseCapture) -> Self {
        self.mouse_capture = capture;
        self
    }

    /// Enable or disable bracketed paste mode.
    pub fn bracketed_paste(mut self, enabled: bool) -> Self {
        self.enable_bracketed_paste = enabled;
        self
    }

    /// Run the application.
    ///
    /// This method:
    /// 1. Enters terminal raw mode
    /// 2. Sets up the event loop
    /// 3. Runs the app until exit
    /// 4. Cleans up terminal state
    ///
    /// Returns `Ok(())` on clean exit, or an error if something fails.
    pub async fn run(self) -> Result<(), A::Error> {
        let Self {
            mut app,
            theme,
            tick_rate,
            external_events,
            mouse_capture,
            enable_bracketed_paste,
        } = self;

        // Enter terminal mode
        let _session = TerminalSession::enter(enable_bracketed_paste, mouse_capture)?;

        // Set up renderer
        let mut renderer = Renderer::new(io::stdout(), theme);
        let size = terminal_size().unwrap_or((80, 24));
        renderer.on_resize(size);

        // Spawn terminal event task
        let terminal_rx = spawn_terminal_event_task();

        // Run the event loop
        run_loop(
            &mut app,
            &mut renderer,
            terminal_rx,
            external_events,
            tick_rate,
        )
        .await
    }
}

/// Convenience function to run an app with default settings.
///
/// Equivalent to `Runner::new(app).run().await`.
pub async fn run<A: App>(app: A) -> Result<(), A::Error> {
    Runner::new(app).run().await
}

/// Internal event loop implementation.
#[cfg_attr(test, allow(dead_code))]
pub(crate) async fn run_loop<A: App, W: Write>(
    app: &mut A,
    renderer: &mut Renderer<W>,
    mut terminal_event_rx: mpsc::UnboundedReceiver<CrosstermEvent>,
    mut external_event_rx: Option<mpsc::UnboundedReceiver<A::Event>>,
    tick_rate: Option<Duration>,
) -> Result<(), A::Error> {
    // Initial render
    renderer.render_frame(|ctx| app.view(ctx))?;

    let mut tick = tick_rate.map(new_tick_interval);

    loop {
        let tick_fut = async {
            if !app.wants_tick() {
                std::future::pending::<()>().await;
            }

            match tick.as_mut() {
                Some(t) => {
                    t.tick().await;
                }
                None => std::future::pending::<()>().await,
            }
        };

        let external_fut = async {
            match external_event_rx.as_mut() {
                Some(rx) => rx.recv().await,
                None => std::future::pending::<Option<A::Event>>().await,
            }
        };

        tokio::select! {
            terminal_event = terminal_event_rx.recv() => {
                let Some(event) = terminal_event else {
                    return Ok(());
                };

                match event {
                    CrosstermEvent::Resize(cols, rows) => {
                        renderer.on_resize((cols, rows));
                        if handle_event(app, renderer, AppEvent::Resize(renderer.context().size)).await? {
                            return Ok(());
                        }
                    }
                    CrosstermEvent::Key(key)
                        if matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat) =>
                    {
                        if handle_event(app, renderer, AppEvent::Key(key)).await? {
                            return Ok(());
                        }
                    }
                    CrosstermEvent::Paste(text) => {
                        if handle_event(app, renderer, AppEvent::Paste(text)).await? {
                            return Ok(());
                        }
                    }
                    CrosstermEvent::Mouse(mouse) => {
                        if handle_event(app, renderer, AppEvent::Mouse(mouse)).await? {
                            return Ok(());
                        }
                    }
                    _ => {}
                }
            }

            app_event = external_fut => {
                match app_event {
                    Some(event) => {
                        if handle_event(app, renderer, AppEvent::External(event)).await? {
                            return Ok(());
                        }
                    }
                    None => {
                        external_event_rx = None;
                    }
                }
            }

            _ = tick_fut => {
                if handle_event(app, renderer, AppEvent::Tick(Instant::now())).await? {
                    return Ok(());
                }
            }
        }
    }
}

/// Dispatch a single event: update → process effects → render.
/// Returns `true` if the app should exit.
async fn handle_event<A: App, W: Write>(
    app: &mut A,
    renderer: &mut Renderer<W>,
    event: AppEvent<A::Event>,
) -> Result<bool, A::Error> {
    let ctx = renderer.context();
    let response = app.update(event, &ctx);
    if response.is_exit() {
        return Ok(true);
    }
    if process_effects(app, renderer, response).await? {
        return Ok(true);
    }
    renderer.render_frame(|ctx| app.view(ctx))?;
    Ok(false)
}

/// Process effects from update, returning true if app should exit.
async fn process_effects<A: App, W: Write>(
    app: &mut A,
    renderer: &mut Renderer<W>,
    response: Response<A::Effect>,
) -> Result<bool, A::Error> {
    let mut queue: VecDeque<A::Effect> = response.into_messages().into();

    while let Some(effect) = queue.pop_front() {
        // Render before running effect
        renderer.render_frame(|ctx| app.view(ctx))?;

        let follow_up = app.run_effect(&mut renderer.terminal(), effect).await?;
        if follow_up.is_exit() {
            return Ok(true);
        }
        queue.extend(follow_up.into_messages());
    }

    Ok(false)
}

fn new_tick_interval(tick_rate: Duration) -> Interval {
    let mut interval = time::interval(tick_rate);
    interval.set_missed_tick_behavior(time::MissedTickBehavior::Skip);
    interval
}
