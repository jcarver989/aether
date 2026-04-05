use clap::Parser;
use rmcp::{
    ErrorData as McpError, RoleServer, ServerHandler,
    handler::server::{
        router::tool::ToolRouter,
        wrapper::{Json, Parameters},
    },
    model::{
        GetPromptRequestParams, GetPromptResult, Implementation, ListPromptsResult, PaginatedRequestParams, Prompt,
        PromptArgument, PromptMessage, PromptMessageRole, ServerCapabilities, ServerInfo,
    },
    service::RequestContext,
    tool, tool_handler, tool_router,
};
use std::io;
use std::path::PathBuf;
use std::sync::Arc;
use std::{collections::HashMap, fmt::Display};
use std::{fs, path::Path};
use tokio::sync::RwLock;
use utils::substitution::substitute_parameters;

use super::tools::{
    LoadSkillsInput, LoadSkillsOutput, SaveNoteInput, SaveNoteOutput, SearchNotesInput, SearchNotesOutput, SkillFile,
    SkillRequest, save_note,
};
use crate::skills::tools::search_notes::search_notes;
use aether_project::{PromptCatalog, SKILL_FILENAME};

/// CLI arguments for `SkillsMcp` server
#[derive(Debug, Clone, Parser)]
pub struct SkillsMcpArgs {
    /// Base directories for skills (each contains a 'skills' subdirectory).
    /// Can be specified multiple times: --dir a --dir b
    /// On skill name collision, the last directory wins.
    #[arg(long = "dir")]
    pub dirs: Vec<PathBuf>,
}

impl SkillsMcpArgs {
    pub fn from_args(args: Vec<String>) -> Result<Self, String> {
        let mut full_args = vec!["skills-mcp".to_string()];
        full_args.extend(args);

        Self::try_parse_from(full_args).map_err(|e| format!("Failed to parse SkillsMcp arguments: {e}"))
    }
}

#[doc = include_str!("../docs/skills_mcp.md")]
#[derive(Clone)]
pub struct SkillsMcp {
    skills_dirs: Vec<PathBuf>,
    notes_dir: PathBuf,
    catalog: Arc<RwLock<PromptCatalog>>,
    tool_router: ToolRouter<Self>,
    roots: Arc<RwLock<Vec<PathBuf>>>,
}

#[derive(Debug)]
enum SkillFileError {
    SkillNotFound(String),
    AbsolutePath,
    TraversalAttempt,
    EscapeAttempt,
    IsDirectory,
    FileNotFound(PathBuf),
    IoError(io::Error),
    InvalidUtf8,
}

impl Display for SkillFileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SkillFileError::SkillNotFound(name) => write!(f, "Skill not found: {name}"),
            SkillFileError::AbsolutePath => write!(f, "Absolute paths are not allowed"),
            SkillFileError::TraversalAttempt => write!(f, "Path traversal (..) is not allowed"),
            SkillFileError::EscapeAttempt => write!(f, "Resolved path escapes skill directory"),
            SkillFileError::IsDirectory => write!(f, "Path is a directory, not a file"),
            SkillFileError::FileNotFound(path) => write!(f, "File not found: {}", path.display()),
            SkillFileError::IoError(e) => write!(f, "IO error: {e}"),
            SkillFileError::InvalidUtf8 => write!(f, "File content is not valid UTF-8"),
        }
    }
}

impl From<io::Error> for SkillFileError {
    fn from(e: io::Error) -> Self {
        SkillFileError::IoError(e)
    }
}

impl SkillsMcp {
    pub fn new(base_dirs: Vec<PathBuf>) -> Self {
        let base_dirs = if base_dirs.is_empty() { vec![PathBuf::from(".")] } else { base_dirs };

        let skills_dirs: Vec<PathBuf> = base_dirs.iter().map(|d| d.join("skills")).collect();
        let notes_dir = base_dirs[0].join("notes");
        let catalog = PromptCatalog::from_dirs(&skills_dirs);

        Self {
            skills_dirs,
            notes_dir,
            catalog: Arc::new(RwLock::new(catalog)),
            tool_router: Self::tool_router(),
            roots: Arc::new(RwLock::new(base_dirs)),
        }
    }

    pub fn from_args(args: Vec<String>) -> Result<Self, String> {
        let parsed_args = SkillsMcpArgs::from_args(args)?;
        Ok(Self::new(parsed_args.dirs))
    }

    pub fn with_roots(mut self, roots: Vec<PathBuf>) -> Self {
        self.roots = Arc::new(RwLock::new(roots));
        self
    }

    fn build_instructions(catalog: &PromptCatalog) -> String {
        let mut instructions = include_str!("./instructions.md").to_string();
        let agent_skills: Vec<_> = catalog.skills().filter(|s| !s.agent_authored).collect();

        if !agent_skills.is_empty() {
            instructions.push_str("\n\n## Complete List of Available Skills\n");
            instructions.push_str("You have access to the following Skills:\n\n");
            for skill in agent_skills {
                use std::fmt::Write as _;
                if skill.tags.is_empty() {
                    let _ = writeln!(instructions, "- **{}**: {}", skill.name, skill.description);
                } else {
                    let tags = skill.tags.join(", ");
                    let _ = writeln!(instructions, "- **{}** [{}]: {}", skill.name, tags, skill.description);
                }
            }
        }

        instructions
    }

    fn find_skill_dir(&self, skill_name: &str) -> Option<PathBuf> {
        self.skills_dirs.iter().rev().find(|d| d.join(skill_name).is_dir()).map(|d| d.join(skill_name))
    }

    fn resolve_skill_file(&self, request: &SkillRequest) -> Result<(PathBuf, String), SkillFileError> {
        let skill_dir =
            self.find_skill_dir(&request.name).ok_or_else(|| SkillFileError::SkillNotFound(request.name.clone()))?;

        let relative_path = request.path.as_deref().unwrap_or(SKILL_FILENAME);
        let resolved_path = Self::validate_path(&skill_dir, relative_path)?;
        Ok((resolved_path, relative_path.to_string()))
    }

    fn validate_path(skill_dir: &Path, relative_path: &str) -> Result<PathBuf, SkillFileError> {
        if Path::new(relative_path).is_absolute() {
            return Err(SkillFileError::AbsolutePath);
        }

        if relative_path.contains("..") {
            return Err(SkillFileError::TraversalAttempt);
        }

        let canonical_skill_dir = skill_dir.canonicalize().map_err(SkillFileError::IoError)?;
        let file_path = skill_dir.join(relative_path);
        let canonical_file_path = file_path.canonicalize().map_err(|e| match e.kind() {
            io::ErrorKind::NotFound => SkillFileError::FileNotFound(file_path),
            _ => SkillFileError::IoError(e),
        })?;

        if !canonical_file_path.starts_with(&canonical_skill_dir) {
            return Err(SkillFileError::EscapeAttempt);
        }

        if canonical_file_path.is_dir() {
            return Err(SkillFileError::IsDirectory);
        }

        Ok(canonical_file_path)
    }

    fn list_available_files(&self, skill_name: &str) -> Vec<String> {
        fn collect_files(dir: &Path, base: &Path, files: &mut Vec<String>) {
            if let Ok(entries) = fs::read_dir(dir) {
                for entry in entries.flatten() {
                    let name = entry.file_name();
                    let name_str = name.to_string_lossy();
                    if name_str.starts_with('.') {
                        continue;
                    }

                    let path = entry.path();
                    if path.is_dir() {
                        collect_files(&path, base, files);
                    } else if path.is_file() {
                        if name_str == SKILL_FILENAME {
                            continue;
                        }

                        if let Ok(relative) = path.strip_prefix(base) {
                            files.push(relative.to_string_lossy().to_string());
                        }
                    }
                }
            }
        }

        let Some(skill_dir) = self.find_skill_dir(skill_name) else {
            return Vec::new();
        };
        let mut files = Vec::new();
        if skill_dir.is_dir() {
            collect_files(&skill_dir, &skill_dir, &mut files);
            files.sort();
        }
        files
    }

    fn load_skill_file(&self, request: &SkillRequest) -> SkillFile {
        let name = request.name.clone();
        let path = request.path.clone().unwrap_or_else(|| SKILL_FILENAME.to_string());

        let result = self.resolve_skill_file(request).and_then(|(resolved, _)| {
            fs::read_to_string(&resolved).map_err(|e| match e.kind() {
                io::ErrorKind::InvalidData => SkillFileError::InvalidUtf8,
                _ => SkillFileError::IoError(e),
            })
        });

        match result {
            Ok(content) => {
                let available_files =
                    if path == SKILL_FILENAME { self.list_available_files(&name) } else { Vec::new() };
                SkillFile { name, path, content: Some(content), error: None, available_files }
            }
            Err(e) => SkillFile { name, path, content: None, error: Some(e.to_string()), available_files: Vec::new() },
        }
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for SkillsMcp {
    fn get_info(&self) -> ServerInfo {
        // try_read() avoids blocking the synchronous get_info() callback.
        // On contention (only possible during a concurrent tool call), we fall back
        // to an empty catalog — this only affects the MCP handshake instructions,
        // and the tools themselves always read fresh data.
        let instructions = match self.catalog.try_read() {
            Ok(catalog) => Self::build_instructions(&catalog),
            Err(_) => Self::build_instructions(&PromptCatalog::empty()),
        };
        ServerInfo::new(ServerCapabilities::builder().enable_prompts().enable_tools().build())
            .with_server_info(Implementation::new("skills-mcp", "0.1.0"))
            .with_instructions(instructions)
    }

    async fn list_prompts(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListPromptsResult, McpError> {
        let catalog = self.catalog.read().await;
        let prompts = catalog
            .slash_commands()
            .map(|s| {
                let arguments = s.argument_hint.as_ref().map(|hint| {
                    vec![PromptArgument::new("ARGUMENTS").with_description(hint.clone()).with_required(false)]
                });

                Prompt::new(s.name.clone(), Some(s.description.clone()), arguments)
            })
            .collect();

        Ok(ListPromptsResult { prompts, next_cursor: None, meta: None })
    }

    async fn get_prompt(
        &self,
        request: GetPromptRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<GetPromptResult, McpError> {
        let catalog = self.catalog.read().await;
        let spec = catalog
            .slash_commands()
            .find(|s| s.name == request.name.as_str())
            .ok_or_else(|| McpError::invalid_params(format!("Prompt '{}' not found", request.name), None))?;

        let body = spec.body.clone();

        let arguments = request.arguments.as_ref().map(|json_map| {
            json_map
                .iter()
                .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                .collect::<HashMap<String, String>>()
        });

        let content = substitute_parameters(&body, &arguments);
        let messages = vec![PromptMessage::new_text(PromptMessageRole::User, content)];

        Ok(GetPromptResult::new(messages).with_description(spec.description.clone()))
    }
}

#[tool_router]
impl SkillsMcp {
    #[doc = include_str!("tools/get_skills/description.md")]
    #[tool]
    pub async fn get_skills(&self, request: Parameters<LoadSkillsInput>) -> Result<Json<LoadSkillsOutput>, String> {
        let Parameters(args) = request;

        let files: Vec<SkillFile> = args.requests.into_iter().map(|req| self.load_skill_file(&req)).collect();

        Ok(Json(LoadSkillsOutput { files }))
    }

    #[doc = include_str!("tools/save_note/description.md")]
    #[tool]
    pub async fn save_note(&self, request: Parameters<SaveNoteInput>) -> Result<Json<SaveNoteOutput>, String> {
        let Parameters(input) = request;
        let today = today_string();
        let result = save_note(&input, &self.notes_dir, &today).map_err(|e| e.to_string())?;
        Ok(Json(result))
    }

    #[doc = include_str!("tools/search_notes/description.md")]
    #[tool]
    pub async fn search_notes(&self, request: Parameters<SearchNotesInput>) -> Result<Json<SearchNotesOutput>, String> {
        let Parameters(input) = request;
        let result = search_notes(&input, &self.notes_dir).map_err(|e| e.to_string())?;
        Ok(Json(result))
    }
}

fn today_string() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_skill(temp_dir: &TempDir, name: &str, content: &str, aux_files: &[(&str, &str)]) {
        let skill_dir = temp_dir.path().join("skills").join(name);
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(skill_dir.join(SKILL_FILENAME), content).unwrap();
        for (path, content) in aux_files {
            let full_path = skill_dir.join(path);
            if let Some(parent) = full_path.parent() {
                fs::create_dir_all(parent).unwrap();
            }
            fs::write(full_path, content).unwrap();
        }
    }

    #[test]
    fn test_load_skill_file_root() {
        let temp_dir = TempDir::new().unwrap();
        create_skill(
            &temp_dir,
            "test-skill",
            "---\ndescription: Test\nagent-invocable: true\n---\n# Test\n\nContent here.",
            &[],
        );

        let server = SkillsMcp::new(vec![temp_dir.path().to_path_buf()]);
        let result = server.load_skill_file(&SkillRequest { name: "test-skill".to_string(), path: None });

        assert_eq!(result.name, "test-skill");
        assert_eq!(result.path, "SKILL.md");
        assert!(result.content.is_some());
        assert!(result.content.unwrap().contains("Content here."));
        assert!(result.error.is_none());
    }

    #[test]
    fn test_load_skill_file_auxiliary() {
        let temp_dir = TempDir::new().unwrap();
        create_skill(
            &temp_dir,
            "test-skill",
            "---\ndescription: Test\nagent-invocable: true\n---\n# Test",
            &[("traits.md", "# Traits content")],
        );

        let server = SkillsMcp::new(vec![temp_dir.path().to_path_buf()]);
        let result = server
            .load_skill_file(&SkillRequest { name: "test-skill".to_string(), path: Some("traits.md".to_string()) });

        assert_eq!(result.path, "traits.md");
        assert_eq!(result.content.unwrap(), "# Traits content");
        assert!(result.available_files.is_empty());
    }

    #[test]
    fn test_load_skill_file_nested() {
        let temp_dir = TempDir::new().unwrap();
        create_skill(
            &temp_dir,
            "test-skill",
            "---\ndescription: Test\nagent-invocable: true\n---\n# Test",
            &[("references/REF.md", "# Reference")],
        );

        let server = SkillsMcp::new(vec![temp_dir.path().to_path_buf()]);
        let result = server.load_skill_file(&SkillRequest {
            name: "test-skill".to_string(),
            path: Some("references/REF.md".to_string()),
        });

        assert_eq!(result.path, "references/REF.md");
        assert_eq!(result.content.unwrap(), "# Reference");
    }

    #[test]
    fn test_reject_absolute_path() {
        let temp_dir = TempDir::new().unwrap();
        create_skill(&temp_dir, "test-skill", "---\ndescription: Test\nagent-invocable: true\n---\n# Test", &[]);

        let server = SkillsMcp::new(vec![temp_dir.path().to_path_buf()]);
        let result = server
            .load_skill_file(&SkillRequest { name: "test-skill".to_string(), path: Some("/etc/passwd".to_string()) });

        assert!(result.error.unwrap().contains("Absolute paths"));
    }

    #[test]
    fn test_reject_traversal() {
        let temp_dir = TempDir::new().unwrap();
        create_skill(&temp_dir, "test-skill", "---\ndescription: Test\nagent-invocable: true\n---\n# Test", &[]);

        let server = SkillsMcp::new(vec![temp_dir.path().to_path_buf()]);
        let result = server.load_skill_file(&SkillRequest {
            name: "test-skill".to_string(),
            path: Some("../other-skill/SKILL.md".to_string()),
        });

        assert!(result.error.unwrap().contains("traversal"));
    }

    #[test]
    fn test_reject_directory() {
        let temp_dir = TempDir::new().unwrap();
        create_skill(&temp_dir, "test-skill", "---\ndescription: Test\nagent-invocable: true\n---\n# Test", &[]);

        let server = SkillsMcp::new(vec![temp_dir.path().to_path_buf()]);
        let result =
            server.load_skill_file(&SkillRequest { name: "test-skill".to_string(), path: Some(".".to_string()) });

        assert!(result.error.unwrap().contains("directory"));
    }

    #[test]
    fn test_available_files() {
        let temp_dir = TempDir::new().unwrap();
        create_skill(
            &temp_dir,
            "test-skill",
            "---\ndescription: Test\nagent-invocable: true\n---\n# Test",
            &[
                ("traits.md", "# Traits"),
                ("error-handling.md", "# Errors"),
                ("references/REF.md", "# Ref"),
                (".hidden", "should be ignored"),
            ],
        );

        let server = SkillsMcp::new(vec![temp_dir.path().to_path_buf()]);
        let result = server.load_skill_file(&SkillRequest { name: "test-skill".to_string(), path: None });

        assert_eq!(result.available_files.len(), 3);
        assert!(result.available_files.contains(&"error-handling.md".to_string()));
        assert!(result.available_files.contains(&"references/REF.md".to_string()));
        assert!(result.available_files.contains(&"traits.md".to_string()));
        assert!(!result.available_files.contains(&"SKILL.md".to_string()));
    }

    #[test]
    fn test_skill_not_found() {
        let temp_dir = TempDir::new().unwrap();
        fs::create_dir_all(temp_dir.path().join("skills")).unwrap();

        let server = SkillsMcp::new(vec![temp_dir.path().to_path_buf()]);
        let result = server.load_skill_file(&SkillRequest { name: "nonexistent".to_string(), path: None });

        assert!(result.error.unwrap().contains("not found"));
    }

    #[test]
    fn test_file_not_found() {
        let temp_dir = TempDir::new().unwrap();
        create_skill(&temp_dir, "test-skill", "---\ndescription: Test\nagent-invocable: true\n---\n# Test", &[]);

        let server = SkillsMcp::new(vec![temp_dir.path().to_path_buf()]);
        let result = server.load_skill_file(&SkillRequest {
            name: "test-skill".to_string(),
            path: Some("nonexistent.md".to_string()),
        });

        assert!(result.error.unwrap().contains("not found"));
    }

    #[test]
    fn test_batch_requests() {
        let temp_dir = TempDir::new().unwrap();
        create_skill(
            &temp_dir,
            "rust",
            "---\ndescription: Rust skill\nagent-invocable: true\n---\n# Rust\n\nSee [traits](./traits.md).",
            &[("traits.md", "# Traits")],
        );
        create_skill(&temp_dir, "python", "---\ndescription: Python skill\nagent-invocable: true\n---\n# Python", &[]);

        let server = SkillsMcp::new(vec![temp_dir.path().to_path_buf()]);
        let input = LoadSkillsInput {
            requests: vec![
                SkillRequest { name: "rust".to_string(), path: None },
                SkillRequest { name: "rust".to_string(), path: Some("traits.md".to_string()) },
                SkillRequest { name: "python".to_string(), path: None },
            ],
        };

        let files: Vec<SkillFile> = input.requests.into_iter().map(|req| server.load_skill_file(&req)).collect();

        assert_eq!(files.len(), 3);

        assert_eq!(files[0].name, "rust");
        assert_eq!(files[0].path, "SKILL.md");
        assert!(files[0].content.is_some());
        assert!(files[0].available_files.contains(&"traits.md".to_string()));

        assert_eq!(files[1].name, "rust");
        assert_eq!(files[1].path, "traits.md");
        assert_eq!(files[1].content.as_deref(), Some("# Traits"));
        assert!(files[1].available_files.is_empty());

        assert_eq!(files[2].name, "python");
        assert_eq!(files[2].path, "SKILL.md");
        assert!(files[2].content.is_some());
    }

    #[test]
    fn test_mixed_success_failure() {
        let temp_dir = TempDir::new().unwrap();
        create_skill(&temp_dir, "exists", "---\ndescription: Exists\nagent-invocable: true\n---\n# Exists", &[]);

        let server = SkillsMcp::new(vec![temp_dir.path().to_path_buf()]);
        let input = LoadSkillsInput {
            requests: vec![
                SkillRequest { name: "exists".to_string(), path: None },
                SkillRequest { name: "nonexistent".to_string(), path: None },
                SkillRequest { name: "exists".to_string(), path: Some("missing.md".to_string()) },
            ],
        };

        let files: Vec<SkillFile> = input.requests.into_iter().map(|req| server.load_skill_file(&req)).collect();

        assert_eq!(files.len(), 3);

        assert!(files[0].content.is_some());
        assert!(files[0].error.is_none());

        assert!(files[1].content.is_none());
        assert!(files[1].error.as_ref().unwrap().contains("not found"));

        assert!(files[2].content.is_none());
        assert!(files[2].error.as_ref().unwrap().contains("not found"));
    }

    #[test]
    fn test_multi_dir_last_wins() {
        let dir_a = TempDir::new().unwrap();
        let dir_b = TempDir::new().unwrap();
        create_skill(&dir_a, "rust", "---\ndescription: Rust A\nagent-invocable: true\n---\n# From A", &[]);
        create_skill(&dir_b, "rust", "---\ndescription: Rust B\nagent-invocable: true\n---\n# From B", &[]);

        let server = SkillsMcp::new(vec![dir_a.path().to_path_buf(), dir_b.path().to_path_buf()]);
        let result = server.load_skill_file(&SkillRequest { name: "rust".to_string(), path: None });

        assert!(result.content.as_ref().unwrap().contains("From B"));
    }

    #[test]
    fn test_multi_dir_union() {
        let dir_a = TempDir::new().unwrap();
        let dir_b = TempDir::new().unwrap();
        create_skill(&dir_a, "rust", "---\ndescription: Rust\nagent-invocable: true\n---\n# Rust", &[]);
        create_skill(&dir_b, "python", "---\ndescription: Python\nagent-invocable: true\n---\n# Python", &[]);

        let server = SkillsMcp::new(vec![dir_a.path().to_path_buf(), dir_b.path().to_path_buf()]);

        let rust = server.load_skill_file(&SkillRequest { name: "rust".to_string(), path: None });
        assert!(rust.content.is_some());
        assert!(rust.error.is_none());

        let python = server.load_skill_file(&SkillRequest { name: "python".to_string(), path: None });
        assert!(python.content.is_some());
        assert!(python.error.is_none());
    }

    #[test]
    fn test_multi_dir_missing_skills_subdir() {
        let dir_a = TempDir::new().unwrap();
        let dir_b = TempDir::new().unwrap();
        // dir_a has no skills/ subdirectory
        create_skill(&dir_b, "rust", "---\ndescription: Rust\nagent-invocable: true\n---\n# Rust", &[]);

        let server = SkillsMcp::new(vec![dir_a.path().to_path_buf(), dir_b.path().to_path_buf()]);
        let result = server.load_skill_file(&SkillRequest { name: "rust".to_string(), path: None });

        assert!(result.content.is_some());
        assert!(result.error.is_none());
    }
}
