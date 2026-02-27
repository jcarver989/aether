//! Test helpers for creating temporary projects (Cargo, Node).
//!
//! Gated behind the `testing` feature flag.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::TempDir;

use crate::uri::path_to_uri;

/// Trait for temporary test projects that support adding files and generating URIs.
pub trait TestProject {
    fn root(&self) -> &Path;

    fn add_file(&self, relative_path: &str, content: &str) -> Result<PathBuf, TestProjectError> {
        let path = self.root().join(relative_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&path, content)?;
        Ok(path)
    }

    fn file_uri(&self, relative_path: &str) -> lsp_types::Uri {
        path_to_uri(&self.root().join(relative_path)).expect("Invalid file path")
    }

    fn file_path_str(&self, relative_path: &str) -> String {
        self.root()
            .join(relative_path)
            .to_str()
            .expect("Non-UTF8 path")
            .to_string()
    }
}

/// Error type for test project operations.
#[derive(Debug)]
pub enum TestProjectError {
    Io(std::io::Error),
    CommandFailed { command: String, stderr: String },
}

impl From<std::io::Error> for TestProjectError {
    fn from(e: std::io::Error) -> Self {
        TestProjectError::Io(e)
    }
}

impl std::fmt::Display for TestProjectError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TestProjectError::Io(e) => write!(f, "IO error: {}", e),
            TestProjectError::CommandFailed { command, stderr } => {
                write!(f, "Command '{}' failed:\n{}", command, stderr)
            }
        }
    }
}

impl std::error::Error for TestProjectError {}

/// A temporary Cargo project for testing.
pub struct CargoProject {
    temp_dir: TempDir,
}

impl TestProject for CargoProject {
    fn root(&self) -> &Path {
        self.temp_dir.path()
    }
}

impl CargoProject {
    /// Create a new minimal Cargo project.
    pub fn new(name: &str) -> Result<Self, TestProjectError> {
        let temp_dir = TempDir::new()?;
        let project = Self { temp_dir };
        project.init_cargo_toml(name)?;
        project.init_src_dir()?;
        Ok(project)
    }

    fn init_cargo_toml(&self, name: &str) -> Result<(), TestProjectError> {
        let content = format!(
            r#"[package]
name = "{}"
version = "0.1.0"
edition = "2021"
"#,
            name
        );
        fs::write(self.root().join("Cargo.toml"), content)?;
        Ok(())
    }

    fn init_src_dir(&self) -> Result<(), TestProjectError> {
        let src_dir = self.root().join("src");
        fs::create_dir_all(&src_dir)?;

        let main_content = r#"fn main() {
    println!("Hello, world!");
}
"#;
        fs::write(src_dir.join("main.rs"), main_content)?;
        Ok(())
    }
}

/// A temporary Node.js/TypeScript project for testing.
pub struct NodeProject {
    temp_dir: TempDir,
}

impl TestProject for NodeProject {
    fn root(&self) -> &Path {
        self.temp_dir.path()
    }
}

impl NodeProject {
    /// Create a new minimal Node/TypeScript project.
    ///
    /// Runs `npm install typescript` so typescript-language-server can find tsserver.
    pub fn new(name: &str) -> Result<Self, TestProjectError> {
        let temp_dir = TempDir::new()?;
        let project = Self { temp_dir };
        project.init_package_json(name)?;
        project.init_tsconfig()?;
        project.init_src_dir()?;
        project.install_typescript()?;
        Ok(project)
    }

    fn init_package_json(&self, name: &str) -> Result<(), TestProjectError> {
        let content = format!(
            r#"{{
  "name": "{}",
  "version": "0.1.0"
}}"#,
            name
        );
        fs::write(self.root().join("package.json"), content)?;
        Ok(())
    }

    fn init_tsconfig(&self) -> Result<(), TestProjectError> {
        let content = r#"{
  "compilerOptions": {
    "strict": true,
    "noEmit": true
  }
}"#;
        fs::write(self.root().join("tsconfig.json"), content)?;
        Ok(())
    }

    fn init_src_dir(&self) -> Result<(), TestProjectError> {
        let src_dir = self.root().join("src");
        fs::create_dir_all(&src_dir)?;
        fs::write(src_dir.join("index.ts"), "")?;
        Ok(())
    }

    fn install_typescript(&self) -> Result<(), TestProjectError> {
        let output = Command::new("npm")
            .args(["install", "--save-dev", "typescript"])
            .current_dir(self.root())
            .output()?;

        if !output.status.success() {
            return Err(TestProjectError::CommandFailed {
                command: "npm install --save-dev typescript".to_string(),
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            });
        }
        Ok(())
    }
}
