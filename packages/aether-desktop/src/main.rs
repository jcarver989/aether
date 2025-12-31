use acp_agent::AgentEvent;
use dioxus::core::spawn_forever;
use dioxus::prelude::*;
use tokio::sync::mpsc;

use file_search::FileSearcherCache;
use state::{AgentHandles, AgentRegistry, AgentSession};
use views::Home;

mod acp_agent;
mod components;
mod diff_engine;
mod docker_diff;
mod docker_watcher;
mod error;
mod file_search;
mod file_watcher;
mod hooks;
mod markdown;
mod settings;
mod state;
mod state_machine;
mod syntax;
mod views;

const FAVICON: Asset = asset!("/assets/favicon.ico");
const THEME_CSS: Asset = asset!("/assets/styling/theme.css");
const MAIN_CSS: Asset = asset!("/assets/styling/main.css");
const TAILWIND_CSS: Asset = asset!("/assets/tailwind.css");

/// Global signal for agent sessions - lives at module scope to avoid CopyValue warnings.
pub static AGENTS: GlobalSignal<AgentRegistry> = Signal::global(AgentRegistry::new);
pub static HANDLES: GlobalSignal<AgentHandles> = Signal::global(AgentHandles::new);
/// Global cache of file searchers, keyed by working directory.
/// Multiple agent views with the same cwd share a single searcher.
pub static FILE_SEARCHERS: GlobalSignal<FileSearcherCache> = Signal::global(FileSearcherCache::new);

/// Helper to mutate an agent by ID.
pub fn with_agent_mut<F>(agent_id: &str, f: F)
where
    F: FnOnce(&mut AgentSession),
{
    if let Some(agent) = AGENTS.write().get_mut(agent_id) {
        f(agent);
    }
}

fn main() {
    #[cfg(feature = "desktop")]
    {
        use dioxus::desktop::{Config, WindowBuilder};
        dioxus::LaunchBuilder::desktop()
            .with_cfg(
                Config::new().with_window(
                    WindowBuilder::new()
                        .with_title("Aether Desktop")
                        .with_always_on_top(false),
                ),
            )
            .launch(App);
    }

    #[cfg(not(feature = "desktop"))]
    dioxus::launch(App);
}

/// Sender for agent events, provided via context to child components.
#[derive(Clone)]
pub struct EventChannel(pub mpsc::UnboundedSender<AgentEvent>);

#[component]
fn App() -> Element {
    let event_tx: mpsc::UnboundedSender<AgentEvent> = use_hook(|| {
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        spawn_forever(async move {
            views::run_ui_consumer(event_rx, &AGENTS, &HANDLES).await;
        });

        event_tx
    });
    use_context_provider(|| EventChannel(event_tx));

    rsx! {
        document::Link { rel: "icon", href: FAVICON }
        document::Link { rel: "stylesheet", href: THEME_CSS }
        document::Link { rel: "stylesheet", href: MAIN_CSS }
        document::Link { rel: "stylesheet", href: TAILWIND_CSS }

        Home {}
    }
}
