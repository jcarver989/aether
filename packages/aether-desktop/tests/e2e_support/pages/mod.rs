//! Page objects for e2e tests.
//!
//! These modules provide a complete API for interacting with UI components.
//! Not all methods may be used by current tests but are available for future tests.

#![allow(dead_code)]
#![allow(unused_imports)]

mod agent_view;
mod new_agent_modal;
mod prompt_input;
mod sidebar;

pub use agent_view::AgentViewPage;
pub use new_agent_modal::NewAgentModalPage;
pub use prompt_input::PromptInputPage;
pub use sidebar::SidebarPage;
