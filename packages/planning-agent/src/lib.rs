pub mod claude_code;
pub mod evals;

use serde::{Deserialize, Serialize};

/// PR information stored in pr.json files
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrInfo {
    /// PR number
    pub pr_number: u32,
    /// Base branch (usually "main")
    pub base_branch: String,
    /// Commit SHA on main branch before the PR was merged (when issue existed)
    pub before_commit: String,
    /// Commit SHA on main branch after the PR was merged (when issue was fixed)
    pub after_commit: String,
}
