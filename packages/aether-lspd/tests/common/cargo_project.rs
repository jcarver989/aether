use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

/// Error type for CargoProject operations
#[derive(Debug)]
pub enum CargoProjectError {
    Io(std::io::Error),
    TempDir(std::io::Error),
}

impl std::fmt::Display for CargoProjectError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CargoProjectError::Io(e) => write!(f, "IO error: {}", e),
            CargoProjectError::TempDir(e) => write!(f, "Failed to create temp dir: {}", e),
        }
    }
}

impl std::error::Error for CargoProjectError {}

/// A temporary Cargo project for testing
pub struct CargoProject {
    temp_dir: TempDir,
    #[allow(dead_code)]
    name: String,
}

impl CargoProject {
    /// Create a new minimal Cargo project
    pub fn new(name: &str) -> Result<Self, CargoProjectError> {
        let temp_dir = TempDir::new().map_err(CargoProjectError::TempDir)?;
        let project = Self {
            temp_dir,
            name: name.to_string(),
        };
        project.init_cargo_toml(name)?;
        project.init_src_dir()?;
        Ok(project)
    }

    /// Get the project root path
    pub fn root(&self) -> &Path {
        self.temp_dir.path()
    }

    /// Add a source file with content
    pub fn add_file(
        &self,
        relative_path: &str,
        content: &str,
    ) -> Result<PathBuf, CargoProjectError> {
        let path = self.root().join(relative_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(CargoProjectError::Io)?;
        }
        fs::write(&path, content).map_err(CargoProjectError::Io)?;
        Ok(path)
    }

    /// Convert a file path to a file:// URI
    pub fn file_uri(&self, relative_path: &str) -> lsp_types::Uri {
        let path = self.root().join(relative_path);
        let uri_string = format!("file://{}", path.display());
        uri_string.parse().expect("Invalid URI")
    }

    fn init_cargo_toml(&self, name: &str) -> Result<(), CargoProjectError> {
        let content = format!(
            r#"[package]
name = "{}"
version = "0.1.0"
edition = "2021"
"#,
            name
        );
        fs::write(self.root().join("Cargo.toml"), content).map_err(CargoProjectError::Io)
    }

    fn init_src_dir(&self) -> Result<(), CargoProjectError> {
        let src_dir = self.root().join("src");
        fs::create_dir_all(&src_dir).map_err(CargoProjectError::Io)?;

        let main_content = r#"fn main() {
    println!("Hello, world!");
}
"#;
        fs::write(src_dir.join("main.rs"), main_content).map_err(CargoProjectError::Io)
    }
}
