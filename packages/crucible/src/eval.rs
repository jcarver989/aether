use aether::{
    agent::{UserMessage, agent},
    llm::{StreamingModelProvider, ToolDefinition},
    mcp::run_mcp_task::McpCommand,
};
use std::path::{Path, PathBuf};
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

    /// Copies files from src_path into a new tmp directory
    pub fn local(src_path: impl Into<PathBuf>) -> Result<Self, Box<dyn std::error::Error>> {
        let src_path = src_path.into();
        let tmpdir = tempfile::tempdir()?;

        if src_path.exists() {
            copy_dir_all(&src_path, tmpdir.path())?;
        }

        let path = tmpdir.path().to_path_buf();
        let _ = tmpdir.keep(); // Keep the directory alive
        Ok(Self::Local { path })
    }

    /// Create an empty tmp directory. Useful for simple evals that start with an empty state and only create files
    pub fn empty() -> Result<Self, Box<dyn std::error::Error>> {
        let tmpdir = tempfile::tempdir()?;
        let path = tmpdir.path().to_path_buf();
        let _ = tmpdir.keep(); // Keep the directory alive
        Ok(Self::Local { path })
    }

    /// Clone a git repository into a new tmp directory and checkout the start_commit sha
    ///
    /// # Arguments
    /// * `url` - Git repository URL
    /// * `start_commit` - Commit SHA to checkout
    /// * `gold_commit` - Gold standard commit SHA for comparison
    /// * `subdir` - Optional subdirectory within the repo to use as working directory
    pub fn git_repo(
        url: impl Into<String>,
        start_commit: impl Into<String>,
        gold_commit: impl Into<String>,
        subdir: Option<impl Into<PathBuf>>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let url = url.into();
        let start_commit = start_commit.into();
        let gold_commit = gold_commit.into();
        let tmpdir = tempfile::tempdir()?;

        tracing::info!("Cloning git repo {} at commit {}", url, start_commit);

        let repo = GitRepo::clone(&url, tmpdir.path())?;
        repo.checkout(&start_commit)?;

        let path = if let Some(subdir) = subdir {
            let subdir = subdir.into();
            let working_dir = tmpdir.path().join(&subdir);

            if !working_dir.exists() {
                return Err(format!(
                    "Subdirectory '{}' does not exist in cloned repo",
                    subdir.display()
                )
                .into());
            }

            working_dir
        } else {
            tmpdir.path().to_path_buf()
        };

        let _ = tmpdir.keep(); // Keep the directory alive

        Ok(Self::GitRepo {
            path,
            url,
            start_commit,
            gold_commit,
        })
    }
}

impl Eval {
    /// Create a new eval
    pub fn new(
        name: impl Into<String>,
        prompt: impl Into<String>,
        working_directory: WorkingDirectory,
        assertions: Vec<EvalAssertion>,
    ) -> Self {
        Self {
            name: name.into(),
            prompt: prompt.into(),
            working_directory,
            assertions,
        }
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
                    "Complete the following task:".to_string(),
                    format!("<task>{}</task>", self.prompt.to_string()),
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
                        &self.working_directory,
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
