use std::fs::{self, read_dir, read_to_string};
use std::path::{Path, PathBuf};

use mcp_utils::MarkdownFile;
use serde::{Deserialize, Serialize};

pub const SKILL_FILENAME: &str = "SKILL.md";

pub type SkillsFile = MarkdownFile<SkillsFrontmatter>;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SkillsFrontmatter {
    pub description: String,
}

#[derive(Debug, Clone)]
pub struct SkillMetadata {
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SkillEntry {
    pub id: String,
    pub helpful_count: u32,
    pub harmful_count: u32,
    pub content: String,
}

impl SkillEntry {
    pub fn parse_all(entries_text: &str) -> Vec<Self> {
        if entries_text.is_empty() {
            return Vec::new();
        }

        let mut entries = Vec::new();
        let mut current_id: Option<String> = None;
        let mut current_helpful: u32 = 0;
        let mut current_harmful: u32 = 0;
        let mut current_lines: Vec<&str> = Vec::new();

        for line in entries_text.lines() {
            if let Some(entry) = parse_entry_heading(line) {
                if let Some(id) = current_id.take() {
                    entries.push(SkillEntry {
                        id,
                        helpful_count: current_helpful,
                        harmful_count: current_harmful,
                        content: current_lines.join("\n").trim().to_string(),
                    });
                    current_lines.clear();
                }
                current_id = Some(entry.0);
                current_helpful = entry.1;
                current_harmful = entry.2;
            } else if current_id.is_some() {
                current_lines.push(line);
            }
        }

        if let Some(id) = current_id {
            entries.push(SkillEntry {
                id,
                helpful_count: current_helpful,
                harmful_count: current_harmful,
                content: current_lines.join("\n").trim().to_string(),
            });
        }

        entries
    }

    pub fn render(&self) -> String {
        format!(
            "### {} (+{}/-{})\n{}\n",
            self.id, self.helpful_count, self.harmful_count, self.content
        )
    }

    pub fn confidence(&self) -> f64 {
        f64::from(self.helpful_count)
            / (f64::from(self.helpful_count) + f64::from(self.harmful_count) + 1.0)
    }

    fn generate_id() -> String {
        use std::collections::hash_map::RandomState;
        use std::hash::{BuildHasher, Hasher};

        let s = RandomState::new();
        let mut hasher = s.build_hasher();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        hasher.write_u128(nanos);
        format!("{:06x}", hasher.finish() & 0x00FF_FFFF)
    }
}

pub struct SkillFile {
    dir: PathBuf,
    pub frontmatter: SkillsFrontmatter,
    pub human_content: String,
    pub entries: Vec<SkillEntry>,
}

impl SkillFile {
    pub fn open(dir: &Path) -> Result<Self, SkillFileError> {
        let (frontmatter, body) = Self::read(dir)?;
        let (human_content, entries_text) = Self::split_human_agent(&body);
        let entries = SkillEntry::parse_all(entries_text);

        Ok(Self {
            dir: dir.to_path_buf(),
            frontmatter,
            human_content: human_content.to_string(),
            entries,
        })
    }

    pub fn create(dir: &Path, frontmatter: SkillsFrontmatter) -> Self {
        Self {
            dir: dir.to_path_buf(),
            frontmatter,
            human_content: String::new(),
            entries: Vec::new(),
        }
    }

    pub fn exists(dir: &Path) -> bool {
        dir.join(SKILL_FILENAME).exists()
    }

    pub fn save(&self) -> Result<(), SkillFileError> {
        let body = self.rebuild_body();
        self.write(&body)
    }

    pub fn add_entry(&mut self, content: String) -> String {
        let id = SkillEntry::generate_id();
        self.entries.push(SkillEntry {
            id: id.clone(),
            helpful_count: 0,
            harmful_count: 0,
            content,
        });
        id
    }

    pub fn find_entry_mut(&mut self, id: &str) -> Result<&mut SkillEntry, SkillFileError> {
        self.entries
            .iter_mut()
            .find(|e| e.id == id)
            .ok_or_else(|| SkillFileError::EntryNotFound(id.to_string()))
    }

    pub fn remove_entry(&mut self, id: &str) -> Option<SkillEntry> {
        let pos = self.entries.iter().position(|e| e.id == id)?;
        Some(self.entries.remove(pos))
    }

    fn read(dir: &Path) -> Result<(SkillsFrontmatter, String), SkillFileError> {
        let path = dir.join(SKILL_FILENAME);
        let raw = fs::read_to_string(&path)?;
        let trimmed = raw.trim();

        let rest = trimmed
            .strip_prefix("---")
            .ok_or(SkillFileError::MissingFrontmatter)?;

        let end_pos = rest
            .find("\n---")
            .ok_or(SkillFileError::MissingFrontmatter)?;

        let frontmatter_str = &rest[..end_pos];
        let body = rest[end_pos + 4..].trim().to_string();

        let frontmatter: SkillsFrontmatter = serde_yaml::from_str(frontmatter_str)
            .map_err(|e| SkillFileError::Yaml(e.to_string()))?;

        Ok((frontmatter, body))
    }

    fn write(&self, content: &str) -> Result<(), SkillFileError> {
        fs::create_dir_all(&self.dir)?;

        let yaml = serde_yaml::to_string(&self.frontmatter)
            .map_err(|e| SkillFileError::Yaml(e.to_string()))?;

        let file_content = format!("---\n{yaml}---\n{content}");
        fs::write(self.dir.join(SKILL_FILENAME), file_content)?;
        Ok(())
    }

    fn split_human_agent(body: &str) -> (&str, &str) {
        match body.find(AGENT_ENTRIES_HEADING) {
            Some(pos) => {
                let human = body[..pos].trim_end();
                let entries_start = pos + AGENT_ENTRIES_HEADING.len();
                let entries = if entries_start < body.len() {
                    body[entries_start..].trim()
                } else {
                    ""
                };
                (human, entries)
            }
            None => (body, ""),
        }
    }

    fn rebuild_body(&self) -> String {
        if self.entries.is_empty() {
            return self.human_content.clone();
        }

        let entries_md: String = self
            .entries
            .iter()
            .enumerate()
            .map(|(i, e)| {
                if i > 0 {
                    format!("\n{}", e.render())
                } else {
                    e.render()
                }
            })
            .collect();

        if self.human_content.is_empty() {
            format!("{AGENT_ENTRIES_HEADING}\n\n{entries_md}")
        } else {
            format!("{}\n\n{AGENT_ENTRIES_HEADING}\n\n{entries_md}", self.human_content)
        }
    }
}

impl SkillMetadata {
    pub fn from_dir(dir_path: &Path) -> Option<Self> {
        let skill_file_path = dir_path.join(SKILL_FILENAME);
        let raw_content = read_to_string(&skill_file_path)
            .inspect_err(|e| {
                tracing::warn!(
                    "Failed to read skill file {}: {}",
                    skill_file_path.display(),
                    e
                );
            })
            .ok()?;

        let content = raw_content.trim();
        let frontmatter = if let Some(rest) = content.strip_prefix("---") {
            let end_pos = rest.find("\n---")?;
            let frontmatter_str = &rest[..end_pos];
            serde_yaml::from_str::<SkillsFrontmatter>(frontmatter_str).ok()
        } else {
            None
        };

        let name = dir_path.file_name()?.to_string_lossy().to_string();

        Some(SkillMetadata {
            description: frontmatter
                .as_ref()
                .map(|f| f.description.clone())
                .unwrap_or_default(),
            name,
        })
    }
}

pub fn load_skill_metadata(skills_dir: &Path) -> Vec<SkillMetadata> {
    if !skills_dir.exists() || !skills_dir.is_dir() {
        return Vec::new();
    }

    read_dir(skills_dir)
        .inspect_err(|e| tracing::warn!("Failed to read skills directory: {e}"))
        .ok()
        .map(|entries| {
            entries
                .filter_map(std::result::Result::ok)
                .filter(|e| {
                    let path = e.path();
                    path.is_dir() && !e.file_name().to_string_lossy().starts_with('.')
                })
                .filter_map(|entry| SkillMetadata::from_dir(&entry.path()))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

#[derive(Debug)]
pub enum SkillFileError {
    Io(std::io::Error),
    Yaml(String),
    MissingFrontmatter,
    NotFound(String),
    EntryNotFound(String),
}

impl std::fmt::Display for SkillFileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SkillFileError::Io(e) => write!(f, "IO error: {e}"),
            SkillFileError::Yaml(e) => write!(f, "YAML error: {e}"),
            SkillFileError::MissingFrontmatter => write!(f, "missing YAML frontmatter"),
            SkillFileError::NotFound(name) => write!(f, "skill not found: {name}"),
            SkillFileError::EntryNotFound(id) => write!(f, "entry not found: {id}"),
        }
    }
}

impl std::error::Error for SkillFileError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            SkillFileError::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for SkillFileError {
    fn from(e: std::io::Error) -> Self {
        SkillFileError::Io(e)
    }
}

const AGENT_ENTRIES_HEADING: &str = "## Agent Entries";

fn parse_entry_heading(line: &str) -> Option<(String, u32, u32)> {
    let rest = line.strip_prefix("### ")?;
    let (id, rest) = rest.split_once(' ')?;

    if !id.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }

    let rest = rest.strip_prefix("(+")?;
    let (helpful_str, rest) = rest.split_once("/-")?;
    let harmful_str = rest.strip_suffix(')')?;

    let helpful: u32 = helpful_str.parse().ok()?;
    let harmful: u32 = harmful_str.parse().ok()?;

    Some((id.to_string(), helpful, harmful))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_load_skill_metadata_empty_directory() {
        let temp_dir = TempDir::new().unwrap();
        let result = load_skill_metadata(temp_dir.path());
        assert!(result.is_empty());
    }

    #[test]
    fn test_load_skill_metadata_nonexistent_directory() {
        let result = load_skill_metadata(&PathBuf::from("/nonexistent/path"));
        assert!(result.is_empty());
    }

    #[test]
    fn test_load_skill_metadata_single_skill() {
        let temp_dir = TempDir::new().unwrap();
        create_skill_file(&temp_dir, "test-skill", "A test skill");

        let result = load_skill_metadata(temp_dir.path());
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "test-skill");
        assert_eq!(result[0].description, "A test skill");
    }

    #[test]
    fn test_load_skill_metadata_multiple_skills() {
        let temp_dir = TempDir::new().unwrap();
        create_skill_file(&temp_dir, "skill-1", "First skill");
        create_skill_file(&temp_dir, "skill-2", "Second skill");
        create_skill_file(&temp_dir, "skill-3", "Third skill");

        let result = load_skill_metadata(temp_dir.path());
        assert_eq!(result.len(), 3);

        let names: Vec<_> = result.iter().map(|s| s.name.clone()).collect();
        assert!(names.contains(&"skill-1".to_string()));
        assert!(names.contains(&"skill-2".to_string()));
        assert!(names.contains(&"skill-3".to_string()));
    }

    #[test]
    fn test_load_skill_metadata_skips_directories_without_skill_md() {
        let temp_dir = TempDir::new().unwrap();
        let skill_dir = temp_dir.path().join("empty-skill");
        std::fs::create_dir_all(&skill_dir).unwrap();

        let result = load_skill_metadata(temp_dir.path());
        assert!(result.is_empty());
    }

    #[test]
    fn test_load_skill_metadata_skips_hidden_directories() {
        let temp_dir = TempDir::new().unwrap();
        create_skill_file(&temp_dir, ".archived", "Hidden skill");
        create_skill_file(&temp_dir, "visible-skill", "Visible skill");

        let result = load_skill_metadata(temp_dir.path());
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "visible-skill");
    }

    #[test]
    fn test_skill_info_from_dir_without_frontmatter() {
        let temp_dir = TempDir::new().unwrap();
        let skill_dir = temp_dir.path().join("no-frontmatter");
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(skill_dir.join(SKILL_FILENAME), "# No frontmatter").unwrap();

        let result = SkillMetadata::from_dir(&skill_dir);
        assert!(result.is_some());
        assert_eq!(result.unwrap().description, "");
    }

    #[test]
    fn test_skill_info_from_dir_nonexistent() {
        let result = SkillMetadata::from_dir(PathBuf::from("/nonexistent").as_path());
        assert!(result.is_none());
    }

    #[test]
    fn test_frontmatter_serde_roundtrip() {
        let fm = SkillsFrontmatter {
            description: "A simple skill".to_string(),
        };

        let yaml = serde_yaml::to_string(&fm).unwrap();
        let parsed: SkillsFrontmatter = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(parsed.description, "A simple skill");
    }

    #[test]
    fn test_backward_compat_old_frontmatter() {
        let yaml = "description: An old skill\nagent_authored: true\nhelpful_count: 5\n";
        let parsed: SkillsFrontmatter = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(parsed.description, "An old skill");
    }

    #[test]
    fn test_confidence() {
        let entry = |helpful, harmful| SkillEntry {
            id: "aaa111".to_string(),
            helpful_count: helpful,
            harmful_count: harmful,
            content: String::new(),
        };

        assert!((entry(0, 0).confidence() - 0.0).abs() < f64::EPSILON);
        assert!((entry(7, 1).confidence() - 7.0 / 9.0).abs() < f64::EPSILON);
        assert!((entry(0, 5).confidence() - 0.0).abs() < f64::EPSILON);
        assert!((entry(3, 0).confidence() - 3.0 / 4.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_write_and_read_roundtrip() {
        let temp_dir = TempDir::new().unwrap();
        let dir = temp_dir.path().join("my-skill");

        let mut sf = SkillFile::create(
            &dir,
            SkillsFrontmatter {
                description: "Test skill".to_string(),
            },
        );
        sf.human_content = "# My Skill\n\nSome content here.".to_string();
        sf.save().unwrap();

        let sf2 = SkillFile::open(&dir).unwrap();
        assert_eq!(sf2.frontmatter.description, "Test skill");
        assert!(sf2.human_content.contains("# My Skill"));
        assert!(sf2.human_content.contains("Some content here."));
    }

    #[test]
    fn test_split_human_agent_no_entries() {
        let body = "# My Skill\n\nSome content here.";
        let (human, entries) = SkillFile::split_human_agent(body);
        assert_eq!(human, "# My Skill\n\nSome content here.");
        assert_eq!(entries, "");
    }

    #[test]
    fn test_split_human_agent_with_entries() {
        let body =
            "# My Skill\n\nSome content.\n\n## Agent Entries\n\n### abc123 (+1/-0)\nAn entry.";
        let (human, entries) = SkillFile::split_human_agent(body);
        assert_eq!(human, "# My Skill\n\nSome content.");
        assert!(entries.contains("### abc123 (+1/-0)"));
        assert!(entries.contains("An entry."));
    }

    #[test]
    fn test_split_human_agent_empty_entries() {
        let body = "# My Skill\n\n## Agent Entries\n";
        let (human, entries) = SkillFile::split_human_agent(body);
        assert_eq!(human, "# My Skill");
        assert_eq!(entries, "");
    }

    #[test]
    fn test_parse_entries_single() {
        let text = "### abc123 (+2/-1)\nThis is an entry.\n\nWith two paragraphs.";
        let entries = SkillEntry::parse_all(text);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].id, "abc123");
        assert_eq!(entries[0].helpful_count, 2);
        assert_eq!(entries[0].harmful_count, 1);
        assert!(entries[0].content.contains("This is an entry."));
        assert!(entries[0].content.contains("With two paragraphs."));
    }

    #[test]
    fn test_parse_entries_multiple() {
        let text = "### aaa111 (+3/-0)\nFirst entry.\n\n### bbb222 (+0/-2)\nSecond entry.";
        let entries = SkillEntry::parse_all(text);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].id, "aaa111");
        assert_eq!(entries[0].helpful_count, 3);
        assert_eq!(entries[1].id, "bbb222");
        assert_eq!(entries[1].harmful_count, 2);
    }

    #[test]
    fn test_parse_entries_empty() {
        let entries = SkillEntry::parse_all("");
        assert!(entries.is_empty());
    }

    #[test]
    fn test_render_roundtrip() {
        let entries = vec![
            SkillEntry {
                id: "abc123".to_string(),
                helpful_count: 2,
                harmful_count: 1,
                content: "First entry content.".to_string(),
            },
            SkillEntry {
                id: "def456".to_string(),
                helpful_count: 0,
                harmful_count: 0,
                content: "Second entry content.".to_string(),
            },
        ];

        let rendered: String = entries.iter().map(|e| e.render()).collect();
        let parsed = SkillEntry::parse_all(&rendered);

        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].id, "abc123");
        assert_eq!(parsed[0].helpful_count, 2);
        assert_eq!(parsed[0].harmful_count, 1);
        assert_eq!(parsed[0].content, "First entry content.");
        assert_eq!(parsed[1].id, "def456");
        assert_eq!(parsed[1].content, "Second entry content.");
    }

    #[test]
    fn test_generate_entry_id_format() {
        let id = SkillEntry::generate_id();
        assert_eq!(id.len(), 6);
        assert!(id.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_generate_entry_id_unique() {
        let id1 = SkillEntry::generate_id();
        let id2 = SkillEntry::generate_id();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_parse_entry_heading_valid() {
        assert_eq!(
            parse_entry_heading("### abc123 (+5/-3)"),
            Some(("abc123".to_string(), 5, 3))
        );
    }

    #[test]
    fn test_parse_entry_heading_zeros() {
        assert_eq!(
            parse_entry_heading("### ff00aa (+0/-0)"),
            Some(("ff00aa".to_string(), 0, 0))
        );
    }

    #[test]
    fn test_parse_entry_heading_not_heading() {
        assert_eq!(parse_entry_heading("## abc123 (+5/-3)"), None);
        assert_eq!(parse_entry_heading("Some random text"), None);
    }

    #[test]
    fn test_parse_entry_heading_non_hex_id() {
        assert_eq!(parse_entry_heading("### zzz999 (+0/-0)"), None);
    }

    // --- SkillFile struct tests ---

    #[test]
    fn test_skill_file_create_and_save_roundtrip() {
        let temp_dir = TempDir::new().unwrap();
        let dir = temp_dir.path().join("new-skill");

        let mut sf = SkillFile::create(
            &dir,
            SkillsFrontmatter {
                description: "A new skill".to_string(),
            },
        );
        let id = sf.add_entry("Use iterators.".to_string());
        sf.save().unwrap();

        let sf2 = SkillFile::open(&dir).unwrap();
        assert_eq!(sf2.frontmatter.description, "A new skill");
        assert_eq!(sf2.entries.len(), 1);
        assert_eq!(sf2.entries[0].id, id);
        assert_eq!(sf2.entries[0].content, "Use iterators.");
    }

    #[test]
    fn test_skill_file_open_existing() {
        let temp_dir = TempDir::new().unwrap();
        let dir = temp_dir.path().join("existing");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join(SKILL_FILENAME),
            "---\ndescription: Existing\n---\n# Human\n\n## Agent Entries\n\n### aaa111 (+2/-1)\nA tip.\n",
        ).unwrap();

        let sf = SkillFile::open(&dir).unwrap();
        assert_eq!(sf.frontmatter.description, "Existing");
        assert_eq!(sf.human_content, "# Human");
        assert_eq!(sf.entries.len(), 1);
        assert_eq!(sf.entries[0].id, "aaa111");
        assert_eq!(sf.entries[0].helpful_count, 2);
    }

    #[test]
    fn test_skill_file_find_and_remove_entry() {
        let temp_dir = TempDir::new().unwrap();
        let dir = temp_dir.path().join("rm-test");

        let mut sf = SkillFile::create(
            &dir,
            SkillsFrontmatter {
                description: "test".to_string(),
            },
        );
        let id1 = sf.add_entry("First.".to_string());
        let id2 = sf.add_entry("Second.".to_string());

        // find_entry_mut
        let entry = sf.find_entry_mut(&id1).unwrap();
        entry.helpful_count = 5;
        assert_eq!(sf.entries[0].helpful_count, 5);

        // find_entry_mut missing
        assert!(sf.find_entry_mut("nonexistent").is_err());

        // remove_entry
        let removed = sf.remove_entry(&id1).unwrap();
        assert_eq!(removed.id, id1);
        assert_eq!(sf.entries.len(), 1);
        assert_eq!(sf.entries[0].id, id2);

        // remove nonexistent
        assert!(sf.remove_entry("gone").is_none());
    }

    #[test]
    fn test_skill_file_exists() {
        let temp_dir = TempDir::new().unwrap();
        let dir = temp_dir.path().join("check");
        assert!(!SkillFile::exists(&dir));

        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(SKILL_FILENAME), "---\ndescription: x\n---\n").unwrap();
        assert!(SkillFile::exists(&dir));
    }

    #[test]
    fn test_skill_file_no_entries_omits_heading() {
        let temp_dir = TempDir::new().unwrap();
        let dir = temp_dir.path().join("no-entries");

        let sf = SkillFile::create(
            &dir,
            SkillsFrontmatter {
                description: "empty".to_string(),
            },
        );
        sf.save().unwrap();

        let raw = std::fs::read_to_string(dir.join(SKILL_FILENAME)).unwrap();
        assert!(!raw.contains("## Agent Entries"));
    }

    #[test]
    fn test_skill_file_preserves_human_content() {
        let temp_dir = TempDir::new().unwrap();
        let dir = temp_dir.path().join("preserve");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join(SKILL_FILENAME),
            "---\ndescription: test\n---\n# Human Notes\n\nImportant stuff.\n",
        )
        .unwrap();

        let mut sf = SkillFile::open(&dir).unwrap();
        sf.add_entry("Agent tip.".to_string());
        sf.save().unwrap();

        let raw = std::fs::read_to_string(dir.join(SKILL_FILENAME)).unwrap();
        assert!(raw.contains("# Human Notes"));
        assert!(raw.contains("Important stuff."));
        assert!(raw.contains("## Agent Entries"));
        assert!(raw.contains("Agent tip."));
    }

    fn create_skill_file(temp_dir: &TempDir, name: &str, description: &str) {
        let skill_dir = temp_dir.path().join(name);
        std::fs::create_dir_all(&skill_dir).unwrap();
        let content = format!("---\ndescription: {} \n---\n# Skill Content\n", description);
        std::fs::write(skill_dir.join(SKILL_FILENAME), content).unwrap();
    }
}
