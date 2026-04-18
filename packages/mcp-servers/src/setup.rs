use crate::{
    CodingMcp, CodingMcpArgs, DefaultCodingTools, LspMcp, PlanMcp, SkillsMcp, SubAgentsMcp, SurveyMcp, TasksMcp,
};
use aether_core::mcp::McpBuilder;
use futures::FutureExt;
use mcp_utils::ServiceExt;
use std::path::{Path, PathBuf};
use tracing::{debug, warn};

#[doc = include_str!("docs/mcp_builder_ext.md")]
pub trait McpBuilderExt {
    /// Registers all built-in in-memory MCP server factories and workspace roots.
    fn with_builtin_servers(self, cwd: PathBuf, roots_path: &Path) -> Self;
}

impl McpBuilderExt for McpBuilder {
    fn with_builtin_servers(self, cwd: PathBuf, roots_path: &Path) -> Self {
        let lsp_cwd = cwd.clone();
        self.register_in_memory_server(
            "coding",
            Box::new(move |args, _input| {
                let project_path = cwd.clone();
                async move {
                    let parsed = match CodingMcpArgs::from_args(args) {
                        Ok(args) => args,
                        Err(e) => {
                            warn!("CodingMcp args parse failed: {e}, using defaults");
                            CodingMcpArgs::default()
                        }
                    };
                    let CodingMcpArgs { permission_mode, rules_dirs, .. } = parsed;
                    debug!(
                        "CodingMcp created with LSP, permission_mode={:?}, rules_dirs={}",
                        permission_mode,
                        rules_dirs.len()
                    );
                    CodingMcp::with_tools(DefaultCodingTools::new())
                        .with_lsp(project_path.clone())
                        .with_rules_dirs(rules_dirs)
                        .with_root_dir(project_path)
                        .with_permission_mode(permission_mode)
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
                    LspMcp::new(project_path.clone()).with_root_dir(project_path).into_dyn()
                }
                .boxed()
            }),
        )
        .register_in_memory_server(
            "skills",
            Box::new(|args, _input| {
                async move { SkillsMcp::from_args(args).expect("Failed to parse SkillsMcp args").into_dyn() }.boxed()
            }),
        )
        .register_in_memory_server(
            "subagents",
            Box::new(|args, _input| {
                async move { SubAgentsMcp::from_args(args).expect("Failed to parse SubAgentsMcp args").into_dyn() }
                    .boxed()
            }),
        )
        .register_in_memory_server(
            "survey",
            Box::new(|_args, _input| async move { SurveyMcp::new().into_dyn() }.boxed()),
        )
        .register_in_memory_server("plan", Box::new(|_args, _input| async move { PlanMcp::new().into_dyn() }.boxed()))
        .register_in_memory_server(
            "tasks",
            Box::new(move |args, _input| {
                async move {
                    TasksMcp::from_args(args)
                        .unwrap_or_else(|e| {
                            tracing::warn!("Failed to parse TasksMcp args: {e}, using defaults");
                            TasksMcp::new()
                        })
                        .into_dyn()
                }
                .boxed()
            }),
        )
        .with_roots(vec![roots_path.to_path_buf()])
    }
}
