use crate::components::app::runtime::{PromptContext, apply_app_effect, should_handle_key_event};
use crate::components::app::{App, AppAction, AppEffect};
use crate::runtime_state::RuntimeState;
use crate::tui::{
    RenderContext, Renderer, RuntimeAction, RuntimeApp, RuntimeEvent, RuntimeOptions, run_app,
};
use acp_utils::client::{AcpEvent, AcpPromptHandle};
use agent_client_protocol as acp;
use crossterm::event::Event;
use std::io::Write;

pub(crate) async fn run_terminal_ui(state: RuntimeState) -> Result<(), Box<dyn std::error::Error>> {
    let RuntimeState {
        session_id,
        agent_name,
        config_options,
        auth_methods,
        theme,
        event_rx,
        prompt_handle,
    } = state;

    let mut app = WispRuntimeApp {
        app: App::new(agent_name, &config_options, auth_methods),
        prompt: PromptOwned {
            prompt_handle,
            session_id,
        },
    };

    run_app(
        &mut app,
        Some(event_rx),
        RuntimeOptions {
            theme,
            ..RuntimeOptions::default()
        },
    )
    .await
}

struct WispRuntimeApp {
    app: App,
    prompt: PromptOwned,
}

struct PromptOwned {
    prompt_handle: AcpPromptHandle,
    session_id: acp::SessionId,
}

impl crate::tui::RootComponent for WispRuntimeApp {
    fn render(&mut self, context: &RenderContext) -> crate::tui::Frame {
        self.app.render(context)
    }
}

impl RuntimeApp for WispRuntimeApp {
    type External = AcpEvent;
    type Effect = AppEffect;
    type Error = Box<dyn std::error::Error>;

    fn on_event(
        &mut self,
        event: RuntimeEvent<Self::External>,
        context: &RenderContext,
    ) -> Vec<RuntimeAction<Self::Effect>> {
        match event {
            RuntimeEvent::Terminal(Event::Key(key_event))
                if should_handle_key_event(key_event.kind) =>
            {
                self.app.dispatch(AppAction::Key(key_event), context)
            }
            RuntimeEvent::Terminal(Event::Paste(text)) => {
                self.app.dispatch(AppAction::Paste(text), context)
            }
            RuntimeEvent::Terminal(Event::Resize(cols, rows)) => {
                self.app.dispatch(AppAction::Resize { cols, rows }, context)
            }
            RuntimeEvent::Terminal(_) => vec![],
            RuntimeEvent::Tick(_) => self.app.dispatch(AppAction::Tick, context),
            RuntimeEvent::External(AcpEvent::SessionUpdate(update)) => self
                .app
                .dispatch(AppAction::SessionUpdate(*update), context),
            RuntimeEvent::External(AcpEvent::ExtNotification(notification)) => self
                .app
                .dispatch(AppAction::ExtNotification(notification), context),
            RuntimeEvent::External(AcpEvent::PromptDone(_)) => {
                self.app.dispatch(AppAction::PromptDone, context)
            }
            RuntimeEvent::External(AcpEvent::PromptError(error)) => {
                eprintln!("Prompt error: {error}");
                self.app.dispatch(AppAction::PromptError, context)
            }
            RuntimeEvent::External(AcpEvent::ElicitationRequest {
                params,
                response_tx,
            }) => self.app.dispatch(
                AppAction::ElicitationRequest {
                    params,
                    response_tx,
                },
                context,
            ),
            RuntimeEvent::External(AcpEvent::AuthenticateComplete { method_id }) => self
                .app
                .dispatch(AppAction::AuthenticateComplete { method_id }, context),
            RuntimeEvent::External(AcpEvent::AuthenticateFailed { method_id, error }) => self
                .app
                .dispatch(AppAction::AuthenticateFailed { method_id, error }, context),
            RuntimeEvent::External(AcpEvent::ConnectionClosed) => vec![RuntimeAction::Exit],
        }
    }

    async fn on_effect<W: Write>(
        &mut self,
        renderer: &mut Renderer<W>,
        effect: Self::Effect,
    ) -> Result<Vec<RuntimeAction<Self::Effect>>, Self::Error> {
        apply_app_effect(
            &mut self.app,
            renderer,
            effect,
            Some(PromptContext::new(
                &self.prompt.prompt_handle,
                &self.prompt.session_id,
            )),
        )
        .await
    }
}
