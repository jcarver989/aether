use crate::components::{RenderContext, RootComponent};
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
pub use terminal::{MouseCapture, TerminalSession};

#[cfg(all(test, feature = "testing"))]
mod tests;

#[derive(Debug, Clone)]
pub enum TerminalEvent {
    Key(KeyEvent),
    Paste(String),
    Mouse(crossterm::event::MouseEvent),
}

#[derive(Debug, PartialEq, Eq)]
pub enum Action<T> {
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

    fn wants_tick(&self) -> bool {
        false
    }
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
    let initial_props = app.props(&renderer.context());
    renderer
        .render_with_props(app, &initial_props)
        .map_err(T::Error::from)?;
    let mut previous_props = initial_props;
    let mut last_render_epoch = renderer.render_epoch();

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
            match app_event_rx.as_mut() {
                Some(rx) => rx.recv().await,
                None => std::future::pending::<Option<T::Event>>().await,
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
                        maybe_render(app, renderer, &mut previous_props, &mut last_render_epoch)?;
                    }
                    CrosstermEvent::Key(key)
                        if matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat) =>
                    {
                        let actions = app.on_terminal_event(TerminalEvent::Key(key), &renderer.context());
                        if process_action_queue(app, renderer, actions, &mut previous_props, &mut last_render_epoch).await? {
                            return Ok(());
                        }
                    }
                    CrosstermEvent::Paste(text) => {
                        let actions = app.on_terminal_event(TerminalEvent::Paste(text), &renderer.context());
                        if process_action_queue(app, renderer, actions, &mut previous_props, &mut last_render_epoch).await? {
                            return Ok(());
                        }
                    }
                    CrosstermEvent::Mouse(mouse) => {
                        let actions = app.on_terminal_event(TerminalEvent::Mouse(mouse), &renderer.context());
                        if process_action_queue(app, renderer, actions, &mut previous_props, &mut last_render_epoch).await? {
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
                        if process_action_queue(app, renderer, actions, &mut previous_props, &mut last_render_epoch).await? {
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
                if process_action_queue(app, renderer, actions, &mut previous_props, &mut last_render_epoch).await? {
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

pub async fn process_action_queue<T, U>(
    app: &mut T,
    renderer: &mut Renderer<U>,
    actions: Vec<Action<T::Action>>,
    previous_props: &mut T::Props,
    last_render_epoch: &mut u64,
) -> Result<bool, T::Error>
where
    T: App + ?Sized,
    U: Write,
{
    let mut queue: VecDeque<_> = actions.into();

    while let Some(action) = queue.pop_front() {
        match action {
            Action::Exit => return Ok(true),
            Action::Custom(effect) => {
                maybe_render(app, renderer, previous_props, last_render_epoch)?;

                let follow_up = app.on_action(renderer, effect).await?;
                queue.extend(follow_up);
            }
        }
    }

    maybe_render(app, renderer, previous_props, last_render_epoch)?;

    Ok(false)
}

fn maybe_render<T, U>(
    app: &mut T,
    renderer: &mut Renderer<U>,
    previous_props: &mut T::Props,
    last_render_epoch: &mut u64,
) -> Result<(), T::Error>
where
    T: App + ?Sized,
    U: Write,
{
    let current_props = app.props(&renderer.context());
    let epoch_changed = renderer.render_epoch() != *last_render_epoch;

    if current_props != *previous_props || epoch_changed {
        renderer
            .render_with_props(app, &current_props)
            .map_err(T::Error::from)?;
        *previous_props = current_props;
        *last_render_epoch = renderer.render_epoch();
    }

    Ok(())
}
