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

Each `skill.md` file contains YAML front-matter with a description and markdown content:

```markdown
---
description: Expert Rust engineer for systems programming, TUI apps, and async patterns
---

You are an expert Rust engineer specializing in:
- Systems programming and memory safety
- async/await patterns with tokio
- TUI development with ratatui
...
```

#### Front-Matter Fields

- `description`: (required) Short description shown in resource listings

#### Derived Fields

- `name`: Inferred from the folder name (e.g., `rust-expert/` → `"rust-expert"`)

## MCP Server Implementation

### Capabilities

```json
{
  "capabilities": {
    "resources": {},
    "tools": {}
  }
}
```

The server provides:
- **Resources**: For discovering available skills
- **Tools**: For dynamically loading skills into agent context

### Resource URIs

Skills are identified using the custom `skill://` URI scheme:

```
skill://rust-expert
skill://web-scraping
skill://data-analysis
```

### Resources List

Skills are loaded fresh from disk on every `resources/list` call (no caching).

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
      "description": "Expert Rust engineer for systems programming, TUI apps, and async patterns",
      "mimeType": "text/markdown"
    },
    {
      "uri": "skill://web-scraping",
      "name": "web-scraping",
      "description": "Extract data from websites using modern scraping techniques",
      "mimeType": "text/markdown"
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

### Tools

The server provides a `load_skill` tool that allows the LLM to dynamically load skills into its context:

**Tool Definition:**
```json
{
  "name": "load_skill",
  "description": "Load a skill into the agent's context to gain specialized knowledge or expertise",
  "inputSchema": {
    "type": "object",
    "properties": {
      "uri": {
        "type": "string",
        "description": "The skill URI (e.g., 'skill://rust-expert')"
      }
    },
    "required": ["uri"]
  }
}
```

**Tool Call:**
```json
{
  "method": "tools/call",
  "params": {
    "name": "load_skill",
    "arguments": {
      "uri": "skill://rust-expert"
    }
  }
}
```

**Tool Result:**
```json
{
  "content": [
    {
      "type": "text",
      "text": "You are an expert Rust engineer specializing in:\n- Systems programming and memory safety\n- async/await patterns with tokio\n- TUI development with ratatui\n..."
    }
  ]
}
```

The tool returns the skill content as text, which the LLM can incorporate into its context.

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
    └── tool_handler.rs      # MCP tool operations (load_skill)
```

### Key Types

```rust
// skill.rs
pub struct Skill {
    pub name: String,        // Derived from folder name
    pub description: String, // From front-matter
    pub content: String,     // Skill instructions (without front-matter)
}

impl Skill {
    /// Load skill from directory (reads skill.md and parses front-matter)
    pub fn from_dir(dir_path: PathBuf) -> Result<Self>;

    /// Convert to MCP resource for listing
    pub fn to_resource(&self) -> Resource;

    /// Convert to MCP resource contents for reading
    pub fn to_resource_contents(&self) -> ResourceContents;
}

// resource_handler.rs
pub struct ResourceHandler {
    skills_dir: PathBuf,
}

impl ResourceHandler {
    /// List all skills by scanning directory (no caching)
    pub fn list_resources(&self) -> Result<ListResourcesResult>;

    /// Read a specific skill by URI
    pub fn read_resource(&self, uri: &str) -> Result<ReadResourceResult>;

    /// Load all skills from disk
    fn load_skills(&self) -> Result<Vec<Skill>>;
}

// tool_handler.rs
pub struct ToolHandler {
    skills_dir: PathBuf,
}

impl ToolHandler {
    /// Execute the load_skill tool
    pub fn load_skill(&self, uri: &str) -> Result<CallToolResult>;
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

### How the LLM Discovers and Loads Skills

The LLM can dynamically discover and load skills as needed:

1. **Discovery**: The MCP client exposes available skills via `resources/list`
2. **Selection**: The LLM sees skill descriptions and decides which to load
3. **Loading**: The LLM calls the `load_skill` tool with the desired skill URI
4. **Context**: The tool returns the skill content, which becomes part of the conversation context

**Example Flow:**

```
User: "I need help optimizing this Rust code for performance"

LLM (internal): I should check if there's a Rust expert skill available
  → Sees skill://rust-expert in resources
  → Calls load_skill tool with uri="skill://rust-expert"
  → Receives skill content as tool result
  → Now has Rust expertise in context

LLM: "Looking at your code, I can help optimize it. [provides expert advice]"
```

The LLM decides when to load skills based on the user's needs, making expertise available on-demand.

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
| MCP Type | Prompts | Resources + Tools |
| Purpose | Execute specific commands | Provide contextual knowledge |
| Invocation | `/command args` | LLM calls `load_skill` tool |
| Content | One-time execution | Persistent context |
| Discovery | `prompts/list` | `resources/list` |
| Loading | Automatic via prompt | LLM-driven via tool call |

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
- [ ] Basic MCP server with resources and tools capabilities
- [ ] Skill file parsing (front-matter + markdown)
- [ ] `resources/list` handler (loads skills fresh each call)
- [ ] `resources/read` handler
- [ ] `load_skill` tool implementation

### Phase 2: Aether Integration
- [ ] MCP client support for skills server
- [ ] CLI commands (list, show)
- [ ] Configuration support in mcp.json

### Phase 3: Advanced Features
- [ ] Skill templates with parameters
- [ ] Skill composition (loading multiple skills)
- [ ] Caching with file watching for performance

## Security Considerations

1. **Path Traversal**: Validate skill directories don't escape skills root
2. **File Size Limits**: Limit skill.md file sizes to prevent DoS
3. **Content Validation**: Sanitize/validate skill content
4. **Access Control**: Only expose skills from configured directories
