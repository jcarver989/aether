use crate::agents::AgentConfig;
use crate::agents::AgentRunner;
use crate::agents::AgentRunnerMessage;
use crate::assertions::{
    assert_command_exit_code, assert_file_exists, assert_file_matches, assert_llm_judge,
    assert_tool_call,
};
use crate::git_repo::GitRepo;
use crate::hooks::HookInput;
use crate::{
    evals::assertion::{EvalAssertion, EvalAssertionResult},
    hooks::Hook,
};
use llm::StreamingModelProvider;
use std::path::{Path, PathBuf};
use tokio::sync::mpsc;

pub struct Eval {
    pub name: String,
    pub prompt: String,
    pub working_directory: WorkingDirectory,
    pub assertions: Vec<EvalAssertion>,

    setup_hooks: Vec<Box<dyn Hook>>,
    before_assertions_hooks: Vec<Box<dyn Hook>>,
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
            WorkingDirectory::Local { path } | WorkingDirectory::GitRepo { path, .. } => path,
        }
    }

    /// Copies files from `src_path` into a new tmp directory
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

    /// Clone a git repository into a new tmp directory and checkout the `start_commit` sha
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
        let url: String = url.into();
        let start_commit: String = start_commit.into();
        let gold_commit: String = gold_commit.into();
        let tmpdir = tempfile::tempdir()?;

        tracing::debug!("Cloning git repo {} at commit {}", url, start_commit);

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
            setup_hooks: Vec::new(),
            before_assertions_hooks: Vec::new(),
        }
    }

    pub fn setup(mut self, hook: impl Hook + 'static) -> Self {
        self.setup_hooks.push(Box::new(hook));
        self
    }

    /// Run a hook when the agent completes, but before the eval runs
    pub fn before_assertions(mut self, hook: impl Hook + 'static) -> Self {
        self.before_assertions_hooks.push(Box::new(hook));
        self
    }

    #[tracing::instrument(skip(self, runner, judge_llm, system_prompt), fields(eval_name = %self.name))]
    pub async fn run<R: AgentRunner, U: StreamingModelProvider + 'static>(
        &self,
        runner: &R,
        judge_llm: U,
        system_prompt: Option<&str>,
    ) -> Result<Vec<(EvalAssertion, EvalAssertionResult)>, Box<dyn std::error::Error + Send + Sync>>
    {
        tracing::info!("Running eval: {}", self.name);

        for (i, hook) in self.setup_hooks.iter().enumerate() {
            let span = tracing::debug_span!("setup_hook", hook_index = i);
            let _enter = span.enter();

            hook.run(HookInput {
                working_directory: self.working_directory.path().to_path_buf(),
                messages: Vec::new(),
            })
            .await
            .map_err(|e| format!("Agent setup hook failed: {e}"))?;
        }

        let messages = {
            let (tx, mut rx) = mpsc::channel(100);

            let task_prompt = [
                "Complete the following task:".to_string(),
                format!("<task>{}</task>", self.prompt),
                format!("CRITICAL INSTRUCTIONS: when working on this task, you MUST only operate within this directory: {}", self.working_directory.path().display())
            ].join("\n");

            let config = AgentConfig {
                working_directory: self.working_directory.path(),
                system_prompt,
                task_prompt: &task_prompt,
            };

            // Run the runner and collect messages concurrently
            let runner_task = runner.run(config, tx);
            let message_task = async {
                let mut messages = Vec::new();
                while let Some(msg) = rx.recv().await {
                    if matches!(msg, AgentRunnerMessage::Done) {
                        messages.push(msg);
                        break;
                    }
                    messages.push(msg);
                }
                messages
            };

            let (run_result, messages) = tokio::join!(runner_task, message_task);
            run_result?;
            messages
        };

        for (i, hook) in self.before_assertions_hooks.iter().enumerate() {
            let span = tracing::debug_span!("before_assertions_hook", hook_index = i);
            let _enter = span.enter();

            hook.run(HookInput {
                working_directory: self.working_directory.path().to_path_buf(),
                messages: messages.clone(),
            })
            .await
            .map_err(|e| format!("Agent complete hook failed: {e}"))?;
        }

        let mut results = Vec::new();
        for (i, assertion) in self.assertions.iter().enumerate() {
            let span = tracing::debug_span!("assertion", assertion_index = i);
            let _enter = span.enter();
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
                EvalAssertion::LLMJudge { prompt_builder } => {
                    assert_llm_judge(
                        &self.working_directory,
                        &self.prompt,
                        &messages,
                        prompt_builder.as_ref(),
                        &judge_llm,
                    )
                    .await
                }
                EvalAssertion::ToolCall {
                    name,
                    arguments,
                    count,
                } => assert_tool_call(name, arguments.as_ref(), count.as_ref(), &messages).await,
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
            "Failed to copy directory from {} to {}",
            src.display(),
            dst.display()
        )))
    }
}
