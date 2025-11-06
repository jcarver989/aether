use crate::claude_code::ClaudeCode;
use aether::agent::{AgentError, Prompt};
use crucible::{Eval, EvalAssertion, EvalMetric, WorkingDirectory};
use std::collections::HashMap;
use std::path::PathBuf;

/// Returns all planning-agent evals defined programmatically
pub fn all_evals() -> Result<Vec<Eval>, Box<dyn std::error::Error>> {
    Ok(vec![
        // Joist ORM - Issue 1406: Optional schema configuration
        Eval::new(
            "joist_easy_issue_1406",
            load_agent_prompt("joist/easy/issue-1406")?,
            WorkingDirectory::git_repo(
                "https://github.com/joist-orm/joist-orm",
                "215c15f99380d3864b58201e31a7614c02d2a366",
                "ac4ac099ad0c667020267f16ee81652cf3d4b181",
                None::<&str>,
            )?,
            vec![code_quality_scorer()?],
        )
        .before_assertions(ClaudeCode::new("plan.md")),
    ])
}

fn load_agent_prompt(eval_path: &str) -> Result<String, Box<dyn std::error::Error>> {
    let tests_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests");
    let prompt_path = tests_dir.join("evals").join(eval_path).join("prompt.md");
    let prompt = Prompt::file(prompt_path.to_str().ok_or("Invalid path")?, false).build()?;
    Ok(prompt)
}

/// Uses LLM as a judge to score code quality (vs human code) on a 10 point scale
fn code_quality_scorer() -> Result<EvalAssertion, AgentError> {
    Ok(EvalAssertion::llm_judge(|ctx| {
        let gold_commit = match ctx.working_dir {
            WorkingDirectory::GitRepo { gold_commit, .. } => Some(gold_commit.as_str()),
            _ => None,
        };

        let dir = ctx.working_dir.path().display().to_string();
        let agent_diff = ctx.git_diff(None).unwrap_or_default();
        let gold_diff = ctx.git_diff(gold_commit).unwrap_or_default();

        Prompt::file_with_args(
            "./src/ten_point_scorer.md",
            false,
            HashMap::from([
                ("working_directory".to_string(), dir),
                ("original_task".to_string(), ctx.original_prompt.to_string()),
                ("gold_diff".to_string(), gold_diff),
                ("agent_diff".to_string(), agent_diff),
                ("json_schema".to_string(), EvalMetric::json_schema()),
            ]),
        )
        .build()
        .expect("Failed to build prompt")
    }))
}
