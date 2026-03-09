use crate::component::{RenderContext, RootComponent};
use crate::rendering::renderer::Renderer;
use crate::theme::Theme;
use crossterm::event::{Event as CrosstermEvent, KeyEvent, KeyEventKind, read};
use std::collections::VecDeque;
use std::io::{self, Write};
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::task::spawn_blocking;
use tokio::time::{self, Interval};
pub mod terminal;
pub use terminal::TerminalSession;

#[cfg(all(test, feature = "testing"))]
mod tests;

#[derive(Debug, Clone)]
pub enum TerminalEvent {
    Key(KeyEvent),
    Paste(String),
}

#[derive(Debug, PartialEq, Eq)]
pub enum Action<T> {
    Render,
    Exit,
    Custom(T),
}

pub struct RuntimeOptions {
    pub theme: Theme,
    pub tick_rate: Option<Duration>,
    pub enable_bracketed_paste: bool,
}

impl Default for RuntimeOptions {
    fn default() -> Self {
        Self {
            theme: Theme::default(),
            tick_rate: Some(Duration::from_millis(100)),
            enable_bracketed_paste: true,
        }
    }
}

#[allow(async_fn_in_trait)]
pub trait App: RootComponent {
    type Event;
    type Action;
    type Error: From<io::Error>;

    fn on_terminal_event(
        &mut self,
        event: TerminalEvent,
        context: &RenderContext,
    ) -> Vec<Action<Self::Action>>;

    fn on_tick(&mut self, _context: &RenderContext) -> Vec<Action<Self::Action>> {
        vec![]
    }

    fn on_event(
        &mut self,
        event: Self::Event,
        context: &RenderContext,
    ) -> Vec<Action<Self::Action>>;

    async fn on_action<T: Write>(
        &mut self,
        renderer: &mut Renderer<T>,
        action: Self::Action,
    ) -> Result<Vec<Action<Self::Action>>, Self::Error>;
}

pub fn spawn_terminal_event_task() -> mpsc::UnboundedReceiver<CrosstermEvent> {
    let (tx, rx) = mpsc::unbounded_channel();
    spawn_blocking(move || {
        loop {
            let event = match read() {
                Ok(event) => event,
                Err(e) => {
                    eprintln!("Event read error: {e}");
                    continue;
                }
            };

            if tx.send(event).is_err() {
                break;
            }
        }
    });
    rx
}

pub async fn run_app<T, U: Write>(
    app: &mut T,
    renderer: &mut Renderer<U>,
    mut terminal_event_rx: mpsc::UnboundedReceiver<CrosstermEvent>,
    mut app_event_rx: Option<mpsc::UnboundedReceiver<T::Event>>,
    tick_rate: Option<Duration>,
) -> Result<(), T::Error>
where
    T: App + ?Sized,
{
    renderer.render(app).map_err(T::Error::from)?;
    let mut tick = tick_rate.map(new_tick_interval);

    loop {
        let tick_fut = async {
            match tick.as_mut() {
                Some(t) => t.tick().await,
                None => std::future::pending().await,
            }
        };
        let external_fut = async {
            match app_event_rx.as_mut() {
                Some(rx) => rx.recv().await,
                None => std::future::pending().await,
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
                        renderer.render(app).map_err(T::Error::from)?;
                    }
                    CrosstermEvent::Key(key)
                        if matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat) =>
                    {
                        let actions = app.on_terminal_event(TerminalEvent::Key(key), &renderer.context());
                        if process_actions(app, renderer, actions).await? {
                            return Ok(());
                        }
                    }
                    CrosstermEvent::Paste(text) => {
                        let actions = app.on_terminal_event(TerminalEvent::Paste(text), &renderer.context());
                        if process_actions(app, renderer, actions).await? {
                            return Ok(());
                        }
                    }
                    _ => {}
                }
            }

            app_event = external_fut => {
                match app_event {
                    Some(event) => {
                        let actions = app.on_event(event, &renderer.context());
                        if process_actions(app, renderer, actions).await? {
                            return Ok(());
                        }
                    }
                    None => {
                        app_event_rx = None;
                    }
                }
            }

            _ = tick_fut => {
                let actions = app.on_tick(&renderer.context());
                if process_actions(app, renderer, actions).await? {
                    return Ok(());
                }
            }
        }
    }
}

fn new_tick_interval(tick_rate: Duration) -> Interval {
    let mut interval = time::interval(tick_rate);
    interval.set_missed_tick_behavior(time::MissedTickBehavior::Skip);
    interval
}

async fn process_actions<T, U>(
    app: &mut T,
    renderer: &mut Renderer<U>,
    actions: Vec<Action<T::Action>>,
) -> Result<bool, T::Error>
where
    T: App + ?Sized,
    U: Write,
{
    let mut queue: VecDeque<_> = actions.into();
    let mut render_requested = false;

    while let Some(action) = queue.pop_front() {
        match action {
            Action::Render => render_requested = true,
            Action::Exit => return Ok(true),
            Action::Custom(effect) => {
                if render_requested {
                    renderer.render(app).map_err(T::Error::from)?;
                    render_requested = false;
                }

                let follow_up = app.on_action(renderer, effect).await?;
                queue.extend(follow_up);
            }
        }
    }

    if render_requested {
        renderer.render(app).map_err(T::Error::from)?;
    }

    Ok(false)
}
