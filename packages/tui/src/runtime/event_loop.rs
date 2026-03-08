use super::{RuntimeAction, RuntimeApp, RuntimeEvent};
use crate::rendering::renderer::Renderer;
use crossterm::event::Event;
use std::collections::VecDeque;
use std::io::Write;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tokio::time::{self, Interval};

pub(crate) async fn run_event_loop<App, W>(
    app: &mut App,
    renderer: &mut Renderer<W>,
    mut terminal_event_rx: mpsc::UnboundedReceiver<Event>,
    mut external_rx: Option<mpsc::UnboundedReceiver<App::External>>,
    tick_rate: Option<Duration>,
) -> Result<(), App::Error>
where
    App: RuntimeApp + ?Sized,
    W: Write,
{
    renderer.render(app).map_err(App::Error::from)?;
    let mut tick = tick_rate.map(new_tick_interval);

    loop {
        tokio::select! {
            maybe_event = terminal_event_rx.recv() => {
                let Some(event) = maybe_event else {
                    return Ok(());
                };

                let event = match event {
                    Event::Resize(cols, rows) => {
                        renderer.update_render_context_with((cols, rows));
                        RuntimeEvent::Terminal(Event::Resize(cols, rows))
                    }
                    event => RuntimeEvent::Terminal(event),
                };

                let actions = app.on_event(event, &renderer.context());
                if process_actions(app, renderer, actions).await? {
                    return Ok(());
                }
            }
            _ = async {
                if let Some(interval) = &mut tick {
                    interval.tick().await;
                }
            }, if tick.is_some() => {
                let event = RuntimeEvent::Tick(Instant::now());
                let actions = app.on_event(event, &renderer.context());
                if process_actions(app, renderer, actions).await? {
                    return Ok(());
                }
            }
            maybe_external = async {
                if let Some(receiver) = external_rx.as_mut() {
                    receiver.recv().await
                } else {
                    None
                }
            }, if external_rx.is_some() => {
                match maybe_external {
                    Some(external) => {
                        let event = RuntimeEvent::External(external);
                        let actions = app.on_event(event, &renderer.context());
                        if process_actions(app, renderer, actions).await? {
                            return Ok(());
                        }
                    }
                    None => {
                        external_rx = None;
                    }
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

async fn process_actions<App, W>(
    app: &mut App,
    renderer: &mut Renderer<W>,
    actions: Vec<RuntimeAction<App::Effect>>,
) -> Result<bool, App::Error>
where
    App: RuntimeApp + ?Sized,
    W: Write,
{
    let mut queue: VecDeque<_> = actions.into();
    let mut render_requested = false;

    while let Some(action) = queue.pop_front() {
        match action {
            RuntimeAction::Render => render_requested = true,
            RuntimeAction::Exit => return Ok(true),
            RuntimeAction::Effect(effect) => {
                if render_requested {
                    renderer.render(app).map_err(App::Error::from)?;
                    render_requested = false;
                }

                let follow_up = app.on_effect(renderer, effect).await?;
                queue.extend(follow_up);
            }
        }
    }

    if render_requested {
        renderer.render(app).map_err(App::Error::from)?;
    }

    Ok(false)
}
