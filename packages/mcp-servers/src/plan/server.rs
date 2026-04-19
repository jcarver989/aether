use rmcp::{
    RoleServer, ServerHandler,
    handler::server::{
        router::tool::ToolRouter,
        wrapper::{Json, Parameters},
    },
    model::{
        CreateElicitationRequestParams, ElicitationAction, ElicitationSchema, EnumSchema, Implementation, Meta,
        ServerCapabilities, ServerInfo,
    },
    service::RequestContext,
    tool, tool_handler, tool_router,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};
use std::io::ErrorKind;
use std::path::PathBuf;
use tokio::fs::read_to_string;
use utils::plan_review::{PlanReviewDecision, PlanReviewElicitationMeta};

const DECISION: &str = "decision";
const FEEDBACK: &str = "feedback";

#[doc = include_str!("../docs/plan_mcp.md")]
#[derive(Clone)]
pub struct PlanMcp {
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl PlanMcp {
    pub fn new() -> Self {
        Self { tool_router: Self::tool_router() }
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
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("plan-mcp", "0.1.0"))
            .with_instructions("MCP Server for Plan mode")
    }
}

impl Default for PlanMcp {
    fn default() -> Self {
        Self::new()
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
        }
    }
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
}
