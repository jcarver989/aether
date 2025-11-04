use aether::{
    agent::{UserMessage, agent},
    llm::{StreamingModelProvider, ToolDefinition},
    mcp::run_mcp_task::McpCommand,
};
use serde::{Deserialize, Serialize};
use std::{
    fs::read_to_string,
    path::{Path, PathBuf},
};
use tokio::sync::mpsc::Sender;

use crate::assertions::{
    assert_command_exit_code, assert_file_exists, assert_file_matches, assert_llm_judge,
    assert_tool_call,
};
use crate::eval_assertion::{EvalAssertion, EvalAssertionResult};
use crate::eval_messages::to_eval_messages;
use crate::git_repo::GitRepo;

#[derive(Debug, Clone)]
pub struct Eval {
    pub name: String,
    pub prompt: String,
    pub working_directory: WorkingDirectory,
    pub assertions: Vec<EvalAssertion>,
}

#[derive(Debug, Clone)]
pub enum WorkingDirectory {
    /// Files copied from src/ directory
    Local { path: PathBuf },
    /// Git repository cloned and checked out
    GitRepo {
        path: PathBuf,
        url: String,
        start_commit: String,
        gold_commit: String,
    },
}

impl WorkingDirectory {
    pub fn path(&self) -> &Path {
        match self {
            WorkingDirectory::Local { path } => path,
            WorkingDirectory::GitRepo { path, .. } => path,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct EvalConfig {
    #[serde(default)]
    assertions: Vec<EvalAssertion>,
    #[serde(skip_serializing_if = "Option::is_none")]
    git: Option<GitConfig>,
}

impl EvalConfig {
    /// Load config from a JSON file
    fn from_json_file(path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let content = read_to_string(path)?;
        serde_json::from_str::<EvalConfig>(&content).map_err(|e| e.into())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GitConfig {
    url: String,
    start_commit: String,
    eval_commit: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    subdir: Option<String>,
}

impl Eval {
    /// Load an eval from a directory containing prompt.md and optional eval.json
    pub fn from_path(eval_path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let eval_name = eval_path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| format!("Invalid eval directory name: {eval_path:?}"))?
            .to_string();

        let prompt_path = eval_path.join("prompt.md");
        let prompt = std::fs::read_to_string(&prompt_path)
            .map_err(|e| format!("Failed to read {prompt_path:?}: {e}"))?;

        let config = EvalConfig::from_json_file(&eval_path.join("eval.json"))?;

        // Check for both git config and src/ - this is an error
        let src_dir = eval_path.join("src");
        let has_git_config = config.git.is_some();
        let has_src_dir = src_dir.exists() && src_dir.is_dir();

        if has_git_config && has_src_dir {
            return Err(format!(
                "Eval '{}' has both git config in eval.json and src/ directory. Only one is allowed.",
                eval_name
            ).into());
        }

        let tmpdir = tempfile::tempdir()
            .map_err(|e| format!("Failed to create tmpdir for {eval_name}: {e}"))?;

        let working_directory = if let Some(git_config) = config.git {
            tracing::info!(
                "Cloning git repo {} at commit {} for eval {}",
                git_config.url,
                git_config.start_commit,
                eval_name
            );

            let repo = GitRepo::clone(&git_config.url, tmpdir.path())
                .map_err(|e| format!("Failed to clone repo for {eval_name}: {e}"))?;

            repo.checkout(&git_config.start_commit)
                .map_err(|e| format!("Failed to checkout commit for {eval_name}: {e}"))?;

            let working_dir = if let Some(ref subdir) = git_config.subdir {
                tmpdir.path().join(subdir)
            } else {
                tmpdir.path().to_path_buf()
            };

            if !working_dir.exists() {
                return Err(format!(
                    "Subdirectory '{}' does not exist in cloned repo",
                    git_config.subdir.as_ref().unwrap()
                )
                .into());
            }

            WorkingDirectory::GitRepo {
                path: working_dir,
                url: git_config.url,
                start_commit: git_config.start_commit,
                gold_commit: git_config.eval_commit,
            }
        } else if has_src_dir {
            copy_dir_all(&src_dir, tmpdir.path())
                .map_err(|e| format!("Failed to copy files from {src_dir:?}: {e}"))?;
            WorkingDirectory::Local {
                path: tmpdir.path().to_path_buf(),
            }
        } else {
            return Err(format!(
                "Eval '{}' must have either a git config in eval.json or a src/ directory",
                eval_name
            )
            .into());
        };

        tracing::info!(
            "Loaded eval: {} with {} assertions",
            eval_name,
            config.assertions.len()
        );

        // Keep the tmpdir so it persists after this function
        let _ = tmpdir.keep();

        Ok(Eval {
            name: eval_name,
            prompt,
            working_directory,
            assertions: config.assertions,
        })
    }

    /// Load all evals from a directory with the expected structure:
    /// ```text
    /// dir/
    ///   evals/
    ///     eval-name-1/
    ///       prompt.md
    ///       assertions.json
    ///       src/  (optional)
    ///     eval-name-2/
    ///       ...
    /// ```
    pub fn load_all(base_dir: &Path) -> Result<Vec<Self>, Box<dyn std::error::Error>> {
        let mut evals = Vec::new();
        let evals_dir = base_dir.join("evals");

        if !evals_dir.exists() {
            return Err(format!("Evals directory not found: {evals_dir:?}").into());
        }

        let entries = std::fs::read_dir(&evals_dir)
            .map_err(|e| format!("Failed to read evals directory: {e}"))?;

        for entry in entries.flatten() {
            let eval_path = entry.path();
            if !eval_path.is_dir() {
                continue;
            }

            match Self::from_path(&eval_path) {
                Ok(eval) => evals.push(eval),
                Err(e) => {
                    tracing::warn!("Failed to load eval from {:?}: {}, skipping", eval_path, e);
                    continue;
                }
            }
        }

        if evals.is_empty() {
            return Err("No evals found in directory".into());
        }

        Ok(evals)
    }

    pub async fn run<T: StreamingModelProvider + 'static, U: StreamingModelProvider + 'static>(
        &self,
        llm: T,
        judge_llm: U,
        tool_definitions: Vec<ToolDefinition>,
        mcp_tx: Sender<McpCommand>,
        system_prompt: Option<String>,
    ) -> Result<Vec<(EvalAssertion, EvalAssertionResult)>, Box<dyn std::error::Error + Send + Sync>>
    {
        let span = tracing::info_span!("eval", eval_name = %self.name);
        let _enter = span.enter();

        tracing::info!("Running eval: {}", self.name);

        let messages = {
            let mut agent_builder = agent(llm).tools(mcp_tx, tool_definitions);

            if let Some(prompt) = system_prompt {
                agent_builder = agent_builder.system(&prompt);
            }

            let (tx, rx, _handle) = agent_builder.spawn().await?;

            tx.send(UserMessage::Text {
                content: [
                    self.prompt.to_string(),
                    format!("CRITICAL INSTRUCTIONS: when working on this task, you MUST only operate within this directory: {}", self.working_directory.path().display())].join("\n"),
            })
            .await?;
            to_eval_messages(rx).await
        };

        let mut results = Vec::new();

        for assertion in &self.assertions {
            let result = match assertion {
                EvalAssertion::FileExists { path } => {
                    assert_file_exists(self.working_directory.path(), path)
                }
                EvalAssertion::FileMatches { path, content } => {
                    assert_file_matches(self.working_directory.path(), path, content)
                }
                EvalAssertion::CommandExitCode {
                    command,
                    expected_code,
                } => {
                    assert_command_exit_code(self.working_directory.path(), command, *expected_code)
                        .await
                }
                EvalAssertion::LLMJudge { prompt } => {
                    assert_llm_judge(
                        self.working_directory.path(),
                        &self.prompt,
                        &messages,
                        prompt,
                        &judge_llm,
                    )
                    .await
                }
                EvalAssertion::ToolCall {
                    name,
                    arguments,
                    count,
                } => assert_tool_call(name, arguments.as_ref(), count, &messages).await,
            };

            results.push((assertion.clone(), result));
        }

        Ok(results)
    }
}

fn copy_dir_all(src: &Path, dst: &Path) -> std::io::Result<()> {
    // Keep the directory structure (e.g., src/ -> dst/src/)
    let status = std::process::Command::new("cp")
        .arg("-r")
        .arg(src)
        .arg(dst)
        .status()?;

    if status.success() {
        Ok(())
    } else {
        Err(std::io::Error::other(format!(
            "Failed to copy directory from {src:?} to {dst:?}"
        )))
    }
}
