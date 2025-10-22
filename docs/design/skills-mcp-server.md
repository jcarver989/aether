# Skills MCP Server Design

## Overview

A Model Context Protocol (MCP) server that exposes skills as resources, allowing AI agents to discover and load specialized knowledge/instructions dynamically. Similar to the slash-commands MCP server but uses MCP resources instead of prompts.

## Architecture

### Directory Structure

```
skills/
  ├── rust-expert/
  │   └── skill.md
  ├── web-scraping/
  │   └── skill.md
  └── data-analysis/
      └── skill.md
```

### Skill File Format

Each `skill.md` file contains YAML front-matter with metadata and markdown content:

```markdown
---
name: rust-expert
description: Expert Rust engineer for systems programming, TUI apps, and async patterns
audience: ["assistant"]
priority: 0.9
---

You are an expert Rust engineer specializing in:
- Systems programming and memory safety
- async/await patterns with tokio
- TUI development with ratatui
...
```

#### Front-Matter Fields

- `name`: (required) Unique identifier for the skill
- `description`: (required) Short description shown in resource listings
- `audience`: (optional) Array of `["user", "assistant"]` - defaults to `["assistant"]`
- `priority`: (optional) 0.0-1.0 indicating importance - defaults to 0.5
- `tags`: (optional) Array of tags for categorization

## MCP Server Implementation

### Capabilities

```json
{
  "capabilities": {
    "resources": {
      "subscribe": true,
      "listChanged": true
    }
  }
}
```

### Resource URIs

Skills are identified using the custom `skill://` URI scheme:

```
skill://rust-expert
skill://web-scraping
skill://data-analysis
```

### Resources List

**Request:**
```json
{
  "method": "resources/list"
}
```

**Response:**
```json
{
  "resources": [
    {
      "uri": "skill://rust-expert",
      "name": "rust-expert",
      "title": "Rust Expert",
      "description": "Expert Rust engineer for systems programming, TUI apps, and async patterns",
      "mimeType": "text/markdown",
      "annotations": {
        "audience": ["assistant"],
        "priority": 0.9,
        "lastModified": "2025-10-22T10:30:00Z"
      }
    },
    {
      "uri": "skill://web-scraping",
      "name": "web-scraping",
      "title": "Web Scraping",
      "description": "Extract data from websites using modern scraping techniques",
      "mimeType": "text/markdown",
      "annotations": {
        "audience": ["assistant"],
        "priority": 0.7,
        "lastModified": "2025-10-21T15:00:00Z"
      }
    }
  ]
}
```

### Resource Read

**Request:**
```json
{
  "method": "resources/read",
  "params": {
    "uri": "skill://rust-expert"
  }
}
```

**Response:**
```json
{
  "contents": [
    {
      "uri": "skill://rust-expert",
      "name": "rust-expert",
      "title": "Rust Expert",
      "mimeType": "text/markdown",
      "text": "You are an expert Rust engineer specializing in:\n- Systems programming..."
    }
  ]
}
```

### File System Watching

The server monitors the skills directory for changes and sends `notifications/resources/list_changed` when:
- New skill directories are added
- Skill files are modified
- Skills are removed

## Rust Implementation

### Module Structure

```
aether-skills/
├── Cargo.toml
└── src/
    ├── main.rs              # MCP server binary
    ├── lib.rs               # Library exports
    ├── skill.rs             # Skill struct and parsing
    ├── resource_handler.rs  # MCP resource operations
    └── watcher.rs           # File system watching
```

### Key Types

```rust
// skill.rs
pub struct Skill {
    pub name: String,
    pub description: String,
    pub content: String,
    pub audience: Vec<Audience>,
    pub priority: f64,
    pub tags: Vec<String>,
    pub modified: DateTime<Utc>,
}

pub enum Audience {
    User,
    Assistant,
}

impl Skill {
    pub fn from_file(path: PathBuf) -> Result<Self>;
    pub fn to_resource(&self) -> Resource;
    pub fn to_resource_contents(&self) -> ResourceContents;
}

// resource_handler.rs
pub struct ResourceHandler {
    skills_dir: PathBuf,
    skills: Arc<RwLock<HashMap<String, Skill>>>,
}

impl ResourceHandler {
    pub async fn list_resources(&self, cursor: Option<String>) -> Result<ListResourcesResult>;
    pub async fn read_resource(&self, uri: &str) -> Result<ReadResourceResult>;
    pub async fn reload_skills(&self) -> Result<()>;
}

// watcher.rs
pub struct SkillWatcher {
    handler: Arc<ResourceHandler>,
    notifier: mpsc::Sender<Notification>,
}

impl SkillWatcher {
    pub async fn start(&self) -> Result<()>;
}
```

### Front-Matter Parsing

Use `gray_matter` crate for YAML front-matter extraction:

```rust
use gray_matter::{Matter, engine::YAML};

let matter = Matter::<YAML>::new();
let result = matter.parse(&content);
let metadata: SkillMetadata = serde_yaml::from_str(&result.data)?;
let content = result.content;
```

## Integration with Aether

### Loading Skills into Agent Context

Skills are loaded explicitly by the user or agent:

```rust
// In aether agent
async fn load_skill(&mut self, skill_uri: &str) -> Result<()> {
    let mcp_client = self.mcp_client("skills")?;
    let response = mcp_client.read_resource(skill_uri).await?;

    let content = response.contents.first()
        .ok_or_else(|| anyhow!("No content"))?;

    // Add to agent context using existing Prompt::text
    self.context.push(Prompt::text(
        "system",
        &content.text,
    ));

    Ok(())
}
```

### CLI Commands

Add skill-related commands to aether:

```bash
# List available skills
aether skills list

# Load a skill into current session
aether skills load rust-expert

# Show skill details
aether skills show rust-expert
```

### Configuration

In `mcp.json`:

```json
{
  "mcpServers": {
    "skills": {
      "command": "aether-skills",
      "args": ["--skills-dir", "/path/to/skills"],
      "env": {}
    }
  }
}
```

## Comparison: Skills vs Slash Commands

| Feature | Slash Commands | Skills |
|---------|---------------|--------|
| MCP Type | Prompts | Resources |
| Purpose | Execute specific commands | Provide contextual knowledge |
| Invocation | `/command args` | Explicit load or auto-discovery |
| Content | One-time execution | Persistent context |
| Discovery | `prompts/list` | `resources/list` |
| Updates | Not applicable | `resources/updated` notifications |

## Use Cases

1. **Role Specialization**: Load "rust-expert" skill when working on Rust code
2. **Domain Knowledge**: Load "finance" skill when analyzing financial data
3. **Project Context**: Load project-specific skills automatically
4. **Dynamic Expertise**: Switch between different expert personas based on task

## Future Enhancements

1. **Skill Dependencies**: Skills can reference other skills
2. **Skill Composition**: Combine multiple skills into a composite skill
3. **Auto-loading**: Automatically load skills based on file types or context
4. **Skill Templates**: Parameterized skills with variable substitution
5. **Skill Store**: Central repository for sharing community skills

## Implementation Phases

### Phase 1: Core Server
- [ ] Basic MCP server with resources capability
- [ ] Skill file parsing (front-matter + markdown)
- [ ] resources/list and resources/read handlers
- [ ] File system watching and list_changed notifications

### Phase 2: Aether Integration
- [ ] MCP client integration in aether
- [ ] Prompt::text usage for skill content
- [ ] CLI commands (list, load, show)
- [ ] Configuration support

### Phase 3: Advanced Features
- [ ] Skill subscriptions
- [ ] Auto-loading based on context
- [ ] Skill templates
- [ ] Web UI for skill management

## Security Considerations

1. **Path Traversal**: Validate skill directories don't escape skills root
2. **File Size Limits**: Limit skill.md file sizes to prevent DoS
3. **Content Validation**: Sanitize/validate skill content
4. **Access Control**: Only expose skills from configured directories
