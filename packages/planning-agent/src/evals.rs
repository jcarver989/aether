use aether::agent::Prompt;
use crucible::{Eval, EvalAssertion, WorkingDirectory};
use std::path::PathBuf;

/// Returns all planning-agent evals defined programmatically
pub fn all_evals() -> Result<Vec<Eval>, Box<dyn std::error::Error>> {
    Ok(vec![
        // Joist ORM - Issue 1406: Optional schema configuration
        Eval::new(
            "joist_easy_issue_1406",
            load_prompt("joist/easy/issue-1406")?,
            WorkingDirectory::git_repo(
                "https://github.com/joist-orm/joist-orm",
                "215c15f99380d3864b58201e31a7614c02d2a366",
                "ac4ac099ad0c667020267f16ee81652cf3d4b181",
                None::<&str>,
            )?,
            vec![EvalAssertion::llm_judge(
                "The agent should have written a plan.md file to describe their implementation plan. \
                     The agent was working off a git commit from the past. You have the actual git diff \
                     that real humans wrote to accomplish the task, this is the 'gold standard'. Using the \
                     actual diff as the platonic ideal, score the agent's plan on a scale of 1 to 10 from \
                     the lens of a staff+ engineer and how likely this plan (if followed) would be to produce \
                     the actual code humans wrote.",
            )],
        ),
    ])
}

fn load_prompt(eval_path: &str) -> Result<String, Box<dyn std::error::Error>> {
    let tests_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests");
    let prompt_path = tests_dir.join("evals").join(eval_path).join("prompt.md");
    let prompt = Prompt::file(prompt_path.to_str().ok_or("Invalid path")?, false).build()?;
    Ok(prompt)
}
