use clap::Parser;
use rmcp::{
    ErrorData as McpError, RoleServer, ServerHandler,
    handler::server::{
        router::tool::ToolRouter,
        wrapper::{Json, Parameters},
    },
    model::{
        CreateElicitationRequestParams, ElicitationAction, ElicitationSchema, EnumSchema, GetPromptRequestParams,
        GetPromptResult, Implementation, ListPromptsResult, Meta, PaginatedRequestParams, Prompt, PromptArgument,
        PromptMessage, PromptMessageRole, ServerCapabilities, ServerInfo,
    },
    service::RequestContext,
    tool, tool_handler, tool_router,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::io::ErrorKind;
use std::path::PathBuf;
use tokio::fs::read_to_string;
use tokio::process::Command;
use utils::plan_review::{PlanReviewDecision, PlanReviewElicitationMeta};
use utils::substitution::substitute_parameters;

pub const DEFAULT_PLAN_PROMPT: &str = include_str!("./default_prompt.md");

const DECISION: &str = "decision";
const FEEDBACK: &str = "feedback";
const PROMPT_NAME: &str = "plan";
const ARGUMENTS: &str = "ARGUMENTS";

#[derive(Debug, Clone, Parser)]
#[command(name = "plan-mcp")]
pub struct PlanMcpArgs {
    /// Markdown file whose body is returned as the `plan` MCP prompt.
    /// When the flag is absent or the file is missing at invocation time,
    /// `DEFAULT_PLAN_PROMPT` is used instead.
    #[arg(long)]
    pub prompt_file: Option<PathBuf>,

    /// Command invoked instead of the default MCP elicitation when
    /// `submit_plan` is called. All trailing positional tokens in the
    /// `mcp.json` `args` array become the program + its arguments; the
    /// absolute plan-file path is appended as the final positional arg.
    /// Stdout from the command is returned verbatim to the agent as
    /// feedback.
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub submit_command: Vec<String>,
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
    prompt_file: Option<PathBuf>,
    submit_command: Vec<String>,
}

#[tool_router]
impl PlanMcp {
    pub fn new() -> Self {
        Self { tool_router: Self::tool_router(), prompt_file: None, submit_command: Vec::new() }
    }

    pub fn from_args(args: Vec<String>) -> Result<Self, String> {
        let PlanMcpArgs { prompt_file, submit_command } = PlanMcpArgs::from_args(args)?;
        Ok(Self { tool_router: Self::tool_router(), prompt_file, submit_command })
    }

    pub fn with_prompt_file(mut self, path: PathBuf) -> Self {
        self.prompt_file = Some(path);
        self
    }

    pub fn with_submit_command(mut self, command: Vec<String>) -> Self {
        self.submit_command = command;
        self
    }

    #[doc = include_str!("./submit_plan_description.md")]
    #[tool]
    pub async fn submit_plan(
        &self,
        request: Parameters<SubmitPlanInput>,
        context: RequestContext<RoleServer>,
    ) -> Result<Json<SubmitPlanOutput>, String> {
        let Parameters(input) = request;
        let plan = Plan::load(&input.plan_path).await.map_err(|e| e.to_string())?;

        if !self.submit_command.is_empty() {
            return run_external_submit(&plan, &self.submit_command).await.map(Json).map_err(|e| e.to_string());
        }

        let form = Self::build_elicitation_form(&plan)?;
        let result = context.peer.create_elicitation(form).await.map_err(|e| e.to_string())?;

        if result.action != ElicitationAction::Accept {
            return Ok(Json(SubmitPlanOutput { approved: false, feedback: None }));
        }

        let decision = result
            .content
            .as_ref()
            .and_then(|content| content.get(DECISION))
            .and_then(serde_json::Value::as_str)
            .unwrap_or(PlanReviewDecision::Deny.as_str());

        if decision == PlanReviewDecision::Approve.as_str() {
            return Ok(Json(SubmitPlanOutput { approved: true, feedback: None }));
        }

        let feedback = result
            .content
            .as_ref()
            .and_then(|content| content.get(FEEDBACK))
            .and_then(serde_json::Value::as_str)
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());

        Ok(Json(SubmitPlanOutput { approved: false, feedback }))
    }

    fn build_elicitation_form(plan: &Plan) -> Result<CreateElicitationRequestParams, String> {
        let meta = PlanReviewElicitationMeta::new(&plan.path, &plan.content)
            .to_json()
            .map(Meta)
            .map_err(|e| format!("failed to serialize plan review metadata: {e}"))?;

        let approve = PlanReviewDecision::Approve.as_str();
        let deny = PlanReviewDecision::Deny.as_str();
        let decision_schema = EnumSchema::builder(vec![approve.into(), deny.into()])
            .untitled()
            .with_default(deny)
            .map_err(|e| format!("failed to build decision schema: {e}"))?
            .build();

        Ok(CreateElicitationRequestParams::FormElicitationParams {
            meta: Some(meta),
            message: format!("Approve plan {}? Review the markdown and choose approve or deny.", plan.path.display()),
            requested_schema: ElicitationSchema::builder()
                .required_enum_schema(DECISION, decision_schema)
                .optional_string(FEEDBACK)
                .build()
                .map_err(|e| format!("failed to build schema: {e}"))?,
        })
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for PlanMcp {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_prompts().enable_tools().build())
            .with_server_info(Implementation::new("plan-mcp", "0.1.0"))
            .with_instructions("MCP Server for Plan mode")
    }

    async fn list_prompts(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListPromptsResult, McpError> {
        let prompt = Prompt::new(
            PROMPT_NAME,
            Some("Generate an implementation plan for a task.".to_string()),
            Some(vec![
                PromptArgument::new(ARGUMENTS)
                    .with_description("The task to generate a plan for.".to_string())
                    .with_required(true),
            ]),
        );

        Ok(ListPromptsResult { prompts: vec![prompt], next_cursor: None, meta: None })
    }

    async fn get_prompt(
        &self,
        request: GetPromptRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<GetPromptResult, McpError> {
        if request.name.as_str() != PROMPT_NAME {
            return Err(McpError::invalid_params(format!("Prompt '{}' not found", request.name), None));
        }

        let prompt = match &self.prompt_file {
            Some(path) => read_to_string(path).await.unwrap_or(DEFAULT_PLAN_PROMPT.to_string()),
            None => DEFAULT_PLAN_PROMPT.to_string(),
        };

        let arguments: Option<HashMap<String, String>> = request.arguments.as_ref().map(|json_map| {
            json_map.iter().filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string()))).collect()
        });

        let content = substitute_parameters(&prompt, &arguments);
        let messages = vec![PromptMessage::new_text(PromptMessageRole::User, content)];
        Ok(GetPromptResult::new(messages).with_description("Enter plan mode.".to_string()))
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SubmitPlanInput {
    /// Absolute path to the markdown plan file to review.
    #[serde(alias = "planPath")]
    pub plan_path: String,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SubmitPlanOutput {
    pub approved: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub feedback: Option<String>,
}

#[derive(Debug)]
pub enum SubmitPlanError {
    RelativePath,
    Io { path: PathBuf, source: std::io::Error },
    EmptyPlan(PathBuf),
    EmptySubmitCommand,
    SubmitCommandSpawn { program: String, source: std::io::Error },
    SubmitCommandFailed { program: String, status: std::process::ExitStatus, stderr: String },
}

struct Plan {
    path: PathBuf,
    content: String,
}

impl Plan {
    async fn load(plan_path: &str) -> Result<Self, SubmitPlanError> {
        let path = PathBuf::from(plan_path);
        if !path.is_absolute() {
            return Err(SubmitPlanError::RelativePath);
        }

        let content = read_to_string(&path).await.map_err(|e| SubmitPlanError::from((e, path.clone())))?;
        if content.trim().is_empty() {
            return Err(SubmitPlanError::EmptyPlan(path));
        }

        Ok(Self { path, content })
    }
}

impl Default for PlanMcp {
    fn default() -> Self {
        Self::new()
    }
}

impl From<(std::io::Error, PathBuf)> for SubmitPlanError {
    fn from((source, path): (std::io::Error, PathBuf)) -> Self {
        Self::Io { path, source }
    }
}

impl Display for SubmitPlanError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            SubmitPlanError::RelativePath => {
                write!(f, "planPath must be an absolute path")
            }
            SubmitPlanError::Io { path, source } => match source.kind() {
                ErrorKind::NotFound => write!(f, "Plan file does not exist: {}", path.display()),
                ErrorKind::InvalidData => write!(f, "Plan file is not valid UTF-8: {}", path.display()),
                _ => write!(f, "Failed to read plan file {}: {source}", path.display()),
            },
            SubmitPlanError::EmptyPlan(path) => {
                write!(f, "Plan file is empty: {}", path.display())
            }
            SubmitPlanError::EmptySubmitCommand => {
                write!(f, "submit_command is empty; expected at least a program name")
            }
            SubmitPlanError::SubmitCommandSpawn { program, source } => {
                write!(f, "Failed to spawn submit command `{program}`: {source}")
            }
            SubmitPlanError::SubmitCommandFailed { program, status, stderr } => {
                let stderr_trimmed = stderr.trim();
                if stderr_trimmed.is_empty() {
                    write!(f, "Submit command `{program}` exited with {status}")
                } else {
                    write!(f, "Submit command `{program}` exited with {status}: {stderr_trimmed}")
                }
            }
        }
    }
}

async fn run_external_submit(plan: &Plan, command: &[String]) -> Result<SubmitPlanOutput, SubmitPlanError> {
    let (program, extra_args) = command.split_first().ok_or(SubmitPlanError::EmptySubmitCommand)?;
    let output = Command::new(program)
        .args(extra_args)
        .arg(&plan.path)
        .output()
        .await
        .map_err(|source| SubmitPlanError::SubmitCommandSpawn { program: program.clone(), source })?;

    if !output.status.success() {
        return Err(SubmitPlanError::SubmitCommandFailed {
            program: program.clone(),
            status: output.status,
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        });
    }

    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    Ok(SubmitPlanOutput { approved: false, feedback: Some(stdout) })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[tokio::test]
    async fn resolve_plan_rejects_relative_path() {
        let result = Plan::load("./example-plan.md").await;
        assert!(matches!(result, Err(SubmitPlanError::RelativePath)));
    }

    #[tokio::test]
    async fn resolve_plan_rejects_missing_file() {
        let temp_dir = TempDir::new().expect("tempdir");
        let path = temp_dir.path().join("missing-plan.md");

        let result = Plan::load(path.to_string_lossy().as_ref()).await;
        assert!(matches!(
            result,
            Err(SubmitPlanError::Io { source, .. }) if source.kind() == std::io::ErrorKind::NotFound
        ));
    }

    #[tokio::test]
    async fn resolve_plan_rejects_invalid_utf8() {
        let temp_dir = TempDir::new().expect("tempdir");
        let path = temp_dir.path().join("invalid-plan.md");
        fs::write(&path, vec![0xff, 0xfe, 0xfd]).expect("write invalid bytes");

        let result = Plan::load(path.to_string_lossy().as_ref()).await;
        assert!(matches!(
            result,
            Err(SubmitPlanError::Io { source, .. }) if source.kind() == std::io::ErrorKind::InvalidData
        ));
    }

    #[tokio::test]
    async fn resolve_plan_rejects_empty_file() {
        let temp_dir = TempDir::new().expect("tempdir");
        let path = temp_dir.path().join("empty-plan.md");
        fs::write(&path, "   \n\n\t").expect("write empty markdown");

        let result = Plan::load(path.to_string_lossy().as_ref()).await;
        assert!(matches!(result, Err(SubmitPlanError::EmptyPlan(_))));
    }

    #[test]
    fn from_args_parses_prompt_file() {
        let server = PlanMcp::from_args(vec!["--prompt-file".into(), "/tmp/plan.md".into()]).unwrap();
        assert_eq!(server.prompt_file, Some(PathBuf::from("/tmp/plan.md")));
    }

    #[test]
    fn from_args_empty_is_ok() {
        let server = PlanMcp::from_args(vec![]).unwrap();
        assert_eq!(server.prompt_file, None);
        assert!(server.submit_command.is_empty());
    }

    #[test]
    fn from_args_parses_trailing_submit_command() {
        let server =
            PlanMcp::from_args(vec!["contextbridge".into(), "plan".into(), "--project".into(), "foo".into()]).unwrap();
        assert_eq!(server.submit_command, vec!["contextbridge", "plan", "--project", "foo"]);
    }

    #[test]
    fn from_args_parses_prompt_file_followed_by_submit_command() {
        let server = PlanMcp::from_args(vec![
            "--prompt-file".into(),
            "/tmp/plan.md".into(),
            "contextbridge".into(),
            "plan".into(),
        ])
        .unwrap();
        assert_eq!(server.prompt_file, Some(PathBuf::from("/tmp/plan.md")));
        assert_eq!(server.submit_command, vec!["contextbridge", "plan"]);
    }
}
