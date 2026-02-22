#[cfg(feature = "all")]
pub mod setup;
#[cfg(feature = "all")]
pub use setup::McpBuilderExt;

#[cfg(feature = "coding")]
pub mod coding;

#[cfg(feature = "skills")]
pub mod skills;

#[cfg(feature = "tasks")]
pub mod tasks;

#[cfg(feature = "subagents")]
pub mod subagents;

#[cfg(feature = "survey")]
pub mod survey;

// Re-export primary types for convenience
#[cfg(feature = "coding")]
pub use coding::{CodingMcp, CodingMcpArgs, CodingTools, DefaultCodingTools};

#[cfg(feature = "coding")]
pub use coding::tools::lsp::LspCodingTools;

#[cfg(feature = "skills")]
pub use skills::{SkillsMcp, SkillsMcpArgs};

#[cfg(feature = "tasks")]
pub use tasks::{TasksMcp, TasksMcpArgs};

#[cfg(feature = "subagents")]
pub use subagents::{SubAgentsMcp, SubAgentsMcpArgs};

#[cfg(feature = "survey")]
pub use survey::SurveyMcp;
