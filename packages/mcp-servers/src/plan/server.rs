use crate::plan::tools::{SubmitPlanInput, SubmitPlanOutput, execute_submit_plan};
use clap::Parser;
use rmcp::{
    RoleServer, ServerHandler,
    handler::server::{
        router::tool::ToolRouter,
        wrapper::{Json, Parameters},
    },
    model::{Implementation, ServerCapabilities, ServerInfo},
    service::RequestContext,
    tool, tool_handler, tool_router,
};
use std::path::PathBuf;

#[derive(Debug, Clone, Parser)]
pub struct PlanMcpArgs {
    /// Optional command to launch an external plan reviewer.
    /// Example: --command plannotator
    #[arg(long = "command")]
    pub command: Option<String>,
}

impl PlanMcpArgs {
    pub fn from_args(args: Vec<String>) -> Result<Self, String> {
        let mut full_args = vec!["plan-mcp".to_string()];
        full_args.extend(args);
        Self::try_parse_from(full_args).map_err(|e| format!("Failed to parse PlanMcp arguments: {e}"))
    }
}

#[doc = include_str!("../docs/plan_mcp.md")]
#[derive(Clone)]
pub struct PlanMcp {
    tool_router: ToolRouter<Self>,
    command: Option<String>,
    root_dir: Option<PathBuf>,
}

impl Default for PlanMcp {
    fn default() -> Self {
        Self::new()
    }
}

impl PlanMcp {
    pub fn new() -> Self {
        Self { tool_router: Self::tool_router(), command: None, root_dir: None }
    }

    pub fn from_args(args: Vec<String>) -> Result<Self, String> {
        let parsed = PlanMcpArgs::from_args(args)?;
        Ok(Self::from_parsed_args(parsed))
    }

    pub fn from_args_with_root(args: PlanMcpArgs, root_dir: PathBuf) -> Self {
        Self::from_parsed_args(args).with_root_dir(root_dir)
    }

    pub fn with_root_dir(mut self, root_dir: PathBuf) -> Self {
        self.root_dir = Some(root_dir);
        self
    }

    fn from_parsed_args(args: PlanMcpArgs) -> Self {
        Self { tool_router: Self::tool_router(), command: args.command, root_dir: None }
    }
}

#[tool_router]
impl PlanMcp {
    #[doc = include_str!("./tools/submit_plan/description.md")]
    #[tool]
    pub async fn submit_plan(
        &self,
        request: Parameters<SubmitPlanInput>,
        context: RequestContext<RoleServer>,
    ) -> Result<Json<SubmitPlanOutput>, String> {
        let Parameters(input) = request;

        execute_submit_plan(input, self.command.as_deref(), self.root_dir.as_deref(), &context).await.map(Json)
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for PlanMcp {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("plan-mcp", "0.1.0"))
            .with_instructions(
                "Submit implementation plans for review. Use `submit_plan` with an absolute markdown file path. \
                 The server either invokes an external reviewer command or prompts the user via elicitation.",
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_mcp_args_parse_without_command() {
        let parsed = PlanMcpArgs::from_args(vec![]).expect("parse args");
        assert_eq!(parsed.command, None);
    }

    #[test]
    fn plan_mcp_args_parse_with_command() {
        let parsed = PlanMcpArgs::from_args(vec!["--command".into(), "plannotator".into()]).expect("parse args");
        assert_eq!(parsed.command, Some("plannotator".to_string()));
    }
}
