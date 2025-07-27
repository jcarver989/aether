# Task 002: Configuration Loader Implementation

## Objective
Implement the configuration system to load mcp.json and environment variables.

## Requirements
1. In `src/config/mod.rs`, implement:
   - Structure to represent mcp.json configuration
   - Environment variable parsing for:
     - OPENROUTER_API_KEY
     - OLLAMA_BASE_URL (default: http://localhost:11434)
     - DEFAULT_PROVIDER (default: "openrouter")
     - DEFAULT_MODEL
   - Config loader that reads mcp.json from current directory
   - Error handling for missing/invalid configuration

2. Create the following types:
   ```rust
   pub struct Config {
       pub mcp_servers: HashMap<String, McpServerConfig>,
       pub provider: ProviderType,
       pub model: String,
       pub openrouter_api_key: Option<String>,
       pub ollama_base_url: String,
   }
   
   pub struct McpServerConfig {
       pub command: String,
       pub args: Vec<String>,
       pub env: HashMap<String, String>,
   }
   
   pub enum ProviderType {
       OpenRouter,
       Ollama,
   }
   ```

## Deliverables
- Complete config module implementation
- Unit tests for configuration loading
- Example mcp.json file in project root
- Error types for configuration failures

## Notes
- Use serde for JSON deserialization
- Make configuration errors clear and actionable
- Consider using clap for future CLI argument support