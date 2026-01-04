use dioxus::core::spawn_forever;
use dioxus::prelude::*;

use state::{AgentHandles, AgentRegistry, AgentSession};
use views::Home;

// Native-only modules (require desktop feature)
#[cfg(feature = "desktop")]
mod acp_agent;
#[cfg(feature = "desktop")]
mod diff_engine;
#[cfg(feature = "desktop")]
mod docker_diff;
#[cfg(feature = "desktop")]
mod docker_watcher;
#[cfg(feature = "desktop")]
mod file_search;
#[cfg(feature = "desktop")]
mod file_watcher;

// Fake implementations for web/testing
#[cfg(not(feature = "desktop"))]
mod fakes;

// Cross-platform modules
mod components;
mod error;
mod events;
mod file_types;
mod hooks;
mod markdown;
mod platform;
mod settings;
mod state;
mod state_machine;
mod syntax;
mod views;

// Re-export platform types
use platform::{AgentEvent, FileSearcherCache, mpsc};

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
        let (event_tx, event_rx) = platform::unbounded_channel();
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
