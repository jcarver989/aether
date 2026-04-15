mod error;

use self::error::SubmitPlanError;
use rmcp::{
    RoleServer,
    model::{CreateElicitationRequestParams, ElicitationAction, ElicitationSchema, EnumSchema},
    service::RequestContext,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

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

struct ResolvedPlan {
    path: PathBuf,
    content: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GenericCommandResponse {
    approved: bool,
    feedback: Option<String>,
}

pub async fn execute_submit_plan(
    input: SubmitPlanInput,
    command: Option<&str>,
    root_dir: Option<&Path>,
    context: &RequestContext<RoleServer>,
) -> Result<SubmitPlanOutput, String> {
    let plan = resolve_plan(&input.plan_path).await.map_err(|e| e.to_string())?;

    let review = match command {
        Some(command) => review_with_command(&plan, command, root_dir).await,
        None => elicit_plan_review(context, &plan).await,
    }
    .map_err(|e| e.to_string())?;

    Ok(review)
}

fn workspace_root(root_dir: Option<&Path>) -> Result<PathBuf, SubmitPlanError> {
    root_dir.map_or_else(
        || {
            std::env::current_dir().map_err(|e| {
                SubmitPlanError::WorkspaceRootResolution(format!("failed to determine current directory: {e}"))
            })
        },
        |path| Ok(path.to_path_buf()),
    )
}

async fn resolve_plan(plan_path: &str) -> Result<ResolvedPlan, SubmitPlanError> {
    let path = PathBuf::from(plan_path);
    if !path.is_absolute() {
        return Err(SubmitPlanError::RelativePath);
    }

    if !tokio::fs::try_exists(&path)
        .await
        .map_err(|e| SubmitPlanError::ReadFailed { path: path.clone(), message: e.to_string() })?
    {
        return Err(SubmitPlanError::MissingFile(path));
    }

    let bytes = tokio::fs::read(&path)
        .await
        .map_err(|e| SubmitPlanError::ReadFailed { path: path.clone(), message: e.to_string() })?;

    let content = String::from_utf8(bytes).map_err(|_| SubmitPlanError::InvalidUtf8(path.clone()))?;
    if content.trim().is_empty() {
        return Err(SubmitPlanError::EmptyPlan(path));
    }

    Ok(ResolvedPlan { path, content })
}

async fn review_with_command(
    plan: &ResolvedPlan,
    command: &str,
    root_dir: Option<&Path>,
) -> Result<SubmitPlanOutput, SubmitPlanError> {
    let cwd = workspace_root(root_dir)?;

    let payload = serde_json::json!({
        "protocol": "aether-plan-review/v1",
        "cwd": cwd.display().to_string(),
        "plan_path": plan.path.display().to_string(),
        "permission_mode": "default",
        "tool_input": {
            "plan": &plan.content,
        }
    });
    let payload_json = serde_json::to_vec(&payload)
        .map_err(|e| SubmitPlanError::InvalidCommandResponse(format!("failed to serialize payload: {e}")))?;

    let mut child = Command::new("bash")
        .arg("-lc")
        .arg(command)
        .current_dir(&cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| SubmitPlanError::CommandSpawn(e.to_string()))?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(&payload_json)
            .await
            .map_err(|e| SubmitPlanError::CommandSpawn(format!("failed writing command stdin: {e}")))?;
        stdin
            .write_all(b"\n")
            .await
            .map_err(|e| SubmitPlanError::CommandSpawn(format!("failed finalizing command stdin: {e}")))?;
    }

    let output = child.wait_with_output().await.map_err(|e| SubmitPlanError::CommandSpawn(e.to_string()))?;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if !output.status.success() {
        let exit_code = output.status.code().unwrap_or(-1);
        let combined_output = if stderr.trim().is_empty() {
            stdout
        } else if stdout.trim().is_empty() {
            stderr
        } else {
            format!("stdout:\n{stdout}\n\nstderr:\n{stderr}")
        };

        return Err(SubmitPlanError::CommandFailed {
            command: command.to_string(),
            exit_code,
            output: combined_output,
        });
    }

    parse_command_response(&stdout)
}

async fn elicit_plan_review(
    context: &RequestContext<RoleServer>,
    plan: &ResolvedPlan,
) -> Result<SubmitPlanOutput, SubmitPlanError> {
    let message = format!("Approve plan {}? Review the markdown and choose approve or deny.", plan.path.display());
    let schema = build_plan_review_schema()?;

    let result = context
        .peer
        .create_elicitation(CreateElicitationRequestParams::FormElicitationParams {
            meta: None,
            message,
            requested_schema: schema,
        })
        .await
        .map_err(|e| SubmitPlanError::Elicitation(e.to_string()))?;

    if result.action != ElicitationAction::Accept {
        return Ok(SubmitPlanOutput { approved: false, feedback: None });
    }

    let decision = result
        .content
        .as_ref()
        .and_then(|content| content.get("decision"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or("deny");
    let feedback = normalize_feedback(
        result
            .content
            .as_ref()
            .and_then(|content| content.get("feedback"))
            .and_then(serde_json::Value::as_str)
            .map(ToString::to_string),
    );

    match decision {
        "approve" => Ok(SubmitPlanOutput { approved: true, feedback: None }),
        _ => Ok(SubmitPlanOutput { approved: false, feedback }),
    }
}

fn build_plan_review_schema() -> Result<ElicitationSchema, SubmitPlanError> {
    let decision_schema = EnumSchema::builder(vec!["approve".into(), "deny".into()])
        .untitled()
        .with_default("deny")
        .map_err(|e| SubmitPlanError::Elicitation(format!("failed to build decision schema: {e}")))?
        .build();

    ElicitationSchema::builder()
        .required_enum_schema("decision", decision_schema)
        .optional_string("feedback")
        .build()
        .map_err(|e| SubmitPlanError::Elicitation(format!("failed to build schema: {e}")))
}

fn normalize_feedback(feedback: Option<String>) -> Option<String> {
    feedback.map(|value| value.trim().to_string()).filter(|value| !value.is_empty())
}

fn parse_command_response(stdout: &str) -> Result<SubmitPlanOutput, SubmitPlanError> {
    let trimmed = stdout.trim();
    if trimmed.is_empty() {
        return Err(SubmitPlanError::InvalidCommandResponse("stdout was empty".to_string()));
    }

    if let Ok(response) = serde_json::from_str::<GenericCommandResponse>(trimmed) {
        return Ok(SubmitPlanOutput { approved: response.approved, feedback: normalize_feedback(response.feedback) });
    }

    Err(SubmitPlanError::InvalidCommandResponse(
        "stdout was not recognized as the required JSON response shape {\"approved\": bool, \"feedback\": string|null}"
            .to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[tokio::test]
    async fn resolve_plan_rejects_relative_path() {
        let result = resolve_plan("./example-plan.md").await;
        assert!(matches!(result, Err(SubmitPlanError::RelativePath)));
    }

    #[tokio::test]
    async fn resolve_plan_rejects_missing_file() {
        let temp_dir = TempDir::new().expect("tempdir");
        let path = temp_dir.path().join("missing-plan.md");

        let result = resolve_plan(path.to_string_lossy().as_ref()).await;
        assert!(matches!(result, Err(SubmitPlanError::MissingFile(_))));
    }

    #[tokio::test]
    async fn resolve_plan_rejects_invalid_utf8() {
        let temp_dir = TempDir::new().expect("tempdir");
        let path = temp_dir.path().join("invalid-plan.md");
        fs::write(&path, vec![0xff, 0xfe, 0xfd]).expect("write invalid bytes");

        let result = resolve_plan(path.to_string_lossy().as_ref()).await;
        assert!(matches!(result, Err(SubmitPlanError::InvalidUtf8(_))));
    }

    #[tokio::test]
    async fn resolve_plan_rejects_empty_file() {
        let temp_dir = TempDir::new().expect("tempdir");
        let path = temp_dir.path().join("empty-plan.md");
        fs::write(&path, "   \n\n\t").expect("write empty markdown");

        let result = resolve_plan(path.to_string_lossy().as_ref()).await;
        assert!(matches!(result, Err(SubmitPlanError::EmptyPlan(_))));
    }

    #[test]
    fn parse_command_response_accepts_generic_allow() {
        let parsed = parse_command_response(r#"{ "approved": true }"#).expect("parse response");
        assert!(parsed.approved);
        assert_eq!(parsed.feedback, None);
    }

    #[test]
    fn parse_command_response_accepts_generic_deny_with_feedback() {
        let parsed = parse_command_response(r#"{ "approved": false, "feedback": "Needs more details" }"#)
            .expect("parse response");
        assert!(!parsed.approved);
        assert_eq!(parsed.feedback.as_deref(), Some("Needs more details"));
    }

    #[test]
    fn parse_command_response_rejects_plannotator_hook_shape() {
        let parsed = parse_command_response(
            r#"{ "hookSpecificOutput": { "decision": { "behavior": "deny", "message": "Not enough detail" } } }"#,
        );
        assert!(matches!(parsed, Err(SubmitPlanError::InvalidCommandResponse(_))));
    }

    #[test]
    fn parse_command_response_rejects_invalid_json() {
        let parsed = parse_command_response("not-json");
        assert!(matches!(parsed, Err(SubmitPlanError::InvalidCommandResponse(_))));
    }

    #[test]
    fn parse_command_response_rejects_unknown_shape() {
        let parsed = parse_command_response(r#"{ "ok": true }"#);
        assert!(matches!(parsed, Err(SubmitPlanError::InvalidCommandResponse(_))));
    }
}
