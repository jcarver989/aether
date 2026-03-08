use crate::component::{RenderContext, RootComponent};
use crate::rendering::renderer::Renderer;
use crate::theme::Theme;
use crossterm::event::{Event, read};
use std::io::{self, Write};
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tokio::task::spawn_blocking;

mod event_loop;
mod terminal;

#[cfg(all(test, feature = "testing"))]
mod tests;

#[derive(Debug)]
pub enum RuntimeEvent<E> {
    Terminal(Event),
    Tick(Instant),
    External(E),
}

#[derive(Debug, PartialEq, Eq)]
pub enum RuntimeAction<F> {
    Render,
    Exit,
    Effect(F),
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
pub trait RuntimeApp: RootComponent {
    type External;
    type Effect;
    type Error: From<io::Error>;

    fn on_event(
        &mut self,
        event: RuntimeEvent<Self::External>,
        context: &RenderContext,
    ) -> Vec<RuntimeAction<Self::Effect>>;

    async fn on_effect<W: Write>(
        &mut self,
        renderer: &mut Renderer<W>,
        effect: Self::Effect,
    ) -> Result<Vec<RuntimeAction<Self::Effect>>, Self::Error>;
}

pub fn spawn_terminal_event_task() -> mpsc::UnboundedReceiver<Event> {
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

pub async fn run_app<App>(
    app: &mut App,
    external_rx: Option<mpsc::UnboundedReceiver<App::External>>,
    options: RuntimeOptions,
) -> Result<(), App::Error>
where
    App: RuntimeApp + ?Sized,
{
    let mut session = terminal::TerminalSession::enter(options.enable_bracketed_paste)
        .map_err(App::Error::from)?;
    let mut renderer = Renderer::new(io::stdout(), options.theme);
    renderer.update_render_context();

    let result = event_loop::run_event_loop(
        app,
        &mut renderer,
        spawn_terminal_event_task(),
        external_rx,
        options.tick_rate,
    )
    .await;

    let cleanup_result = session.cleanup().map_err(App::Error::from);
    result?;
    cleanup_result
}
