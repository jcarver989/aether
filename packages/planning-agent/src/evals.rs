use crate::PrInfo;
use crate::claude_code::ClaudeCode;
use aether::agent::{AgentError, substitute_parameters};
use crucible::{Eval, EvalAssertion, EvalMetric, WorkingDirectory};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

const JOIST_ORM_REPO: &str = "https://github.com/joist-orm/joist-orm";

pub fn all_evals() -> Result<Vec<Eval>, Box<dyn std::error::Error>> {
    let tests_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests");
    let evals_dir = tests_dir.join("evals");

    let mut evals = Vec::new();

    for repo_entry in fs::read_dir(&evals_dir)? {
        let repo_path = repo_entry?.path();

        if !repo_path.is_dir() {
            continue;
        }

        for difficulty_entry in fs::read_dir(&repo_path)? {
            let difficulty_entry = difficulty_entry?;
            let difficulty_path = difficulty_entry.path();

            if !difficulty_path.is_dir() {
                continue;
            }

            for issue_entry in fs::read_dir(&difficulty_path)? {
                let issue_entry = issue_entry?;
                let issue_path = issue_entry.path();

                if !issue_path.is_dir() {
                    continue;
                }

                let eval_path = issue_path
                    .strip_prefix(&evals_dir)
                    .map_err(|e| format!("Failed to strip prefix: {}", e))?
                    .to_str()
                    .ok_or("Invalid eval path")?;

                match eval(eval_path, JOIST_ORM_REPO) {
                    Ok(e) => evals.push(e),
                    Err(err) => {
                        eprintln!("Warning: Failed to load eval '{}': {}", eval_path, err)
                    }
                }
            }
        }
    }

    Ok(evals)
}

fn eval(eval_path: &str, repo_url: &str) -> Result<Eval, Box<dyn std::error::Error>> {
    let tests_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests");
    let eval_dir = tests_dir.join("evals").join(eval_path);
    let prompt_path = eval_dir.join("issue.md");
    let prompt = fs::read_to_string(&prompt_path)?;

    let pr_json_path = eval_dir.join("pr.json");
    let pr_json_content = fs::read_to_string(&pr_json_path)?;
    let pr_info: PrInfo = serde_json::from_str(&pr_json_content)?;
    let eval_name = eval_path.replace(['/', '-'], "_");

    Ok(Eval::new(
        &eval_name,
        prompt,
        WorkingDirectory::git_repo(
            repo_url,
            &pr_info.before_commit,
            &pr_info.after_commit,
            None::<&str>,
        )?,
        vec![code_quality_scorer()?],
    )
    .before_assertions(ClaudeCode::new("plan.md")))
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

        let template =
            fs::read_to_string("./src/ten_point_scorer.md").expect("Failed to read scorer prompt");
        let args = Some(HashMap::from([
            ("working_directory".to_string(), dir),
            ("original_task".to_string(), ctx.original_prompt.to_string()),
            ("gold_diff".to_string(), gold_diff),
            ("agent_diff".to_string(), agent_diff),
            ("json_schema".to_string(), EvalMetric::json_schema()),
        ]));
        substitute_parameters(&template, &args)
    }))
}
