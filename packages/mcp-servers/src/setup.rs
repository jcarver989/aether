use crate::{CodingMcp, DefaultCodingTools, LspMcp, SkillsMcp, SubAgentsMcp, SurveyMcp, TasksMcp};
use aether_core::mcp::McpBuilder;
use futures::FutureExt;
use mcp_utils::ServiceExt;
use std::path::{Path, PathBuf};
use tracing::debug;

/// Extension trait that adds built-in MCP server registration to [`McpBuilder`].
pub trait McpBuilderExt {
    /// Registers all built-in in-memory MCP server factories (coding, skills,
    /// subagents, survey, tasks) and workspace roots onto this builder.
    ///
    /// Callers can chain additional configuration (`.with_servers()`,
    /// `.from_json_file()`, etc.) on the returned builder before spawning.
    fn with_builtin_servers(self, cwd: PathBuf, roots_path: &Path) -> Self;
}

impl McpBuilderExt for McpBuilder {
    fn with_builtin_servers(self, cwd: PathBuf, roots_path: &Path) -> Self {
        let tasks_cwd = cwd.clone();
        let lsp_cwd = cwd.clone();
        self.register_in_memory_server(
            "coding",
            Box::new(move |_args, _input| {
                let project_path = cwd.clone();
                async move {
                    debug!("CodingMcp created with LSP for coding server");
                    CodingMcp::with_tools(DefaultCodingTools::new())
                        .with_lsp(project_path.clone())
                        .with_root_dir(project_path)
                        .into_dyn()
                }
                .boxed()
            }),
        )
        .register_in_memory_server(
            "lsp",
            Box::new(move |_args, _input| {
                let project_path = lsp_cwd.clone();
                async move {
                    debug!("LspMcp created with own registry");
                    LspMcp::new(project_path.clone())
                        .with_root_dir(project_path)
                        .into_dyn()
                }
                .boxed()
            }),
        )
        .register_in_memory_server(
            "skills",
            Box::new(|args, _input| {
                async move {
                    SkillsMcp::from_args(args)
                        .expect("Failed to parse SkillsMcp args")
                        .into_dyn()
                }
                .boxed()
            }),
        )
        .register_in_memory_server(
            "subagents",
            Box::new(|args, _input| {
                async move {
                    SubAgentsMcp::from_args(args)
                        .expect("Failed to parse SubAgentsMcp args")
                        .into_dyn()
                }
                .boxed()
            }),
        )
        .register_in_memory_server(
            "survey",
            Box::new(|_args, _input| async move { SurveyMcp::new().into_dyn() }.boxed()),
        )
        .register_in_memory_server(
            "tasks",
            Box::new(move |args, _input| {
                let project_path = tasks_cwd.clone();
                async move {
                    TasksMcp::from_args(args)
                        .unwrap_or_else(|e| {
                            tracing::warn!("Failed to parse TasksMcp args: {e}, using defaults");
                            TasksMcp::new(project_path)
                        })
                        .into_dyn()
                }
                .boxed()
            }),
        )
        .with_roots(vec![roots_path.to_path_buf()])
    }
}
