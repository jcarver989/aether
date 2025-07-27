# Task 004: Configuration Loading

## Overview
Implement comprehensive configuration loading system to support LLM providers, MCP servers, and application settings.

## Missing Components
The new app has partial configuration support but lacks:
- Complete environment variable handling
- MCP server configuration loading from mcp.json
- Provider and model selection logic
- Agent context loading from AGENT.md

## Requirements

### Configuration Sources
- Environment variables for API keys and settings
- mcp.json for MCP server configurations
- AGENT.md for agent context (optional)
- Command line arguments for runtime overrides
- Default values for all settings

### Environment Variables
```
OPENROUTER_API_KEY - OpenRouter API key
OLLAMA_BASE_URL - Ollama server URL (default: http://localhost:11434)
DEFAULT_PROVIDER - LLM provider ("openrouter" or "ollama")
DEFAULT_MODEL - Model name for the provider
RUST_LOG - Logging configuration
```

### Configuration Structure
```rust
pub struct AppConfig {
    pub llm: LlmConfig,
    pub mcp: McpConfig,
    pub agent_context: Option<String>,
    pub ui: UiConfig,
}

pub struct LlmConfig {
    pub provider: ProviderType,
    pub model: String,
    pub openrouter_api_key: Option<String>,
    pub ollama_base_url: String,
}

pub struct McpConfig {
    pub servers: HashMap<String, ServerConfig>,
}

pub struct UiConfig {
    pub tick_rate: f64,
    pub frame_rate: f64,
}
```

### File Loading

#### mcp.json Format
```json
{
  "mcpServers": {
    "filesystem": {
      "command": "npx",
      "args": ["-y", "@modelcontextprotocol/server-filesystem", "/path/to/files"]
    },
    "git": {
      "command": "uvx", 
      "args": ["mcp-server-git", "--repository", "/path/to/repo"]
    }
  }
}
```

#### AGENT.md Loading
- Optional file providing agent context
- Loaded as string and passed to LLM
- Used to customize agent behavior

## Implementation Details

### Configuration Loading Flow
1. Load default values
2. Override with environment variables
3. Load mcp.json if present
4. Load AGENT.md if present
5. Apply command line arguments
6. Validate configuration

### Error Handling
- Missing required environment variables
- Invalid mcp.json format
- File permission errors
- Invalid configuration values
- Graceful degradation when possible

### Integration Points

#### With App Initialization
```rust
// In App::new()
let config = AppConfig::load()?;
let llm_provider = create_provider(&config.llm)?;
let mcp_client = McpClient::with_config(&config.mcp)?;
```

#### With CLI Arguments
```rust
#[derive(Parser)]
pub struct Cli {
    #[arg(long, env = "DEFAULT_PROVIDER")]
    pub provider: Option<String>,
    
    #[arg(long, env = "DEFAULT_MODEL")]
    pub model: Option<String>,
    
    #[arg(long, default_value = "60.0")]
    pub tick_rate: f64,
    
    #[arg(long, default_value = "4.0")]
    pub frame_rate: f64,
}
```

### Validation
- Validate provider types
- Check API key presence for OpenRouter
- Validate Ollama URL format
- Ensure MCP server commands exist
- Check file permissions for AGENT.md

### Runtime Configuration Updates
- Support reloading mcp.json
- Environment variable changes
- Provider switching during runtime
- Configuration persistence

## File Locations
- `mcp.json` - Project root or config directory
- `AGENT.md` - Project root
- Configuration files in standard config directories using `directories` crate

## Integration with Existing Code
- Extend existing `config.rs` module
- Use existing CLI argument parsing
- Integrate with current error handling
- Support existing environment variable names

## Dependencies
- `config` crate for configuration management
- `directories` crate for standard paths
- `serde_json` for JSON parsing
- `clap` for CLI argument parsing

## Acceptance Criteria
- [ ] Complete configuration structure defined
- [ ] Environment variables properly loaded
- [ ] mcp.json parsing implemented
- [ ] AGENT.md loading functional
- [ ] CLI argument integration working
- [ ] Configuration validation in place
- [ ] Error handling for all failure modes
- [ ] Default values for all settings
- [ ] Integration with app initialization
- [ ] Runtime configuration updates supported