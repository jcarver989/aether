use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::State;
use tracing::{info, debug, error};
use aether_core::{
    types::{LlmProvider, OpenRouterConfig, OllamaConfig, ConnectionStatus, ToolDefinition, McpServerStatus},
    mcp::mcp_config::McpServerConfig,
    agent::Agent,
    llm::{openrouter::OpenRouterProvider, ollama::OllamaProvider},
};
use std::collections::HashMap;
use std::path::PathBuf;
use std::fs;
use std::env;
use crate::state::AgentState;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct AppConfig {
    pub active_provider: LlmProvider,
    pub openrouter_config: OpenRouterConfig,
    pub ollama_config: OllamaConfig,
    pub mcp_servers: Vec<McpServerWithId>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct McpServerWithId {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub config: McpServerConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct AppStatus {
    pub connection_status: ConnectionStatus,
    pub available_tools: Vec<ToolDefinition>,
}

/// Get the path to the configuration directory
fn get_config_dir() -> Result<PathBuf, String> {
    dirs::config_dir()
        .map(|dir| dir.join("aether"))
        .ok_or_else(|| "Could not determine config directory".to_string())
}

/// Get the path to the configuration file
fn get_config_file_path() -> Result<PathBuf, String> {
    Ok(get_config_dir()?.join("config.json"))
}

/// Ensure the configuration directory exists
fn ensure_config_dir_exists() -> Result<(), String> {
    let config_dir = get_config_dir()?;
    if !config_dir.exists() {
        fs::create_dir_all(&config_dir)
            .map_err(|e| format!("Failed to create config directory: {}", e))?;
    }
    Ok(())
}

/// Load configuration from file, returning default config if file doesn't exist
fn load_config_from_file() -> Result<AppConfig, String> {
    let config_path = get_config_file_path()?;
    
    if !config_path.exists() {
        // Return default config if file doesn't exist
        return Ok(get_default_config());
    }
    
    let config_content = fs::read_to_string(&config_path)
        .map_err(|e| format!("Failed to read config file: {}", e))?;
    
    // Substitute environment variables in the config content
    let substituted_content = substitute_env_vars(&config_content);
    
    serde_json::from_str(&substituted_content)
        .map_err(|e| format!("Failed to parse config file: {}", e))
}

/// Save configuration to file with atomic write
fn save_config_to_file(config: &AppConfig) -> Result<(), String> {
    ensure_config_dir_exists()?;
    
    let config_path = get_config_file_path()?;
    let temp_path = config_path.with_extension("tmp");
    
    // Serialize config to JSON
    let config_json = serde_json::to_string_pretty(config)
        .map_err(|e| format!("Failed to serialize config: {}", e))?;
    
    // Write to temporary file first (atomic write)
    fs::write(&temp_path, config_json)
        .map_err(|e| format!("Failed to write config file: {}", e))?;
    
    // Move temporary file to final location
    fs::rename(&temp_path, &config_path)
        .map_err(|e| format!("Failed to finalize config file: {}", e))?;
    
    Ok(())
}

/// Substitute environment variables in a string
/// Supports $VAR syntax
fn substitute_env_vars(input: &str) -> String {
    input.split('$').enumerate().map(|(i, part)| {
        if i == 0 {
            part.to_string()
        } else if let Some(end) = part.find(|c: char| !c.is_alphanumeric() && c != '_') {
            let var_name = &part[..end];
            let rest = &part[end..];
            env::var(var_name).unwrap_or_else(|_| format!("${}", var_name)) + rest
        } else {
            env::var(part).unwrap_or_else(|_| format!("${}", part))
        }
    }).collect()
}

/// Get default configuration
fn get_default_config() -> AppConfig {
    AppConfig {
        active_provider: LlmProvider::Ollama,
        openrouter_config: OpenRouterConfig {
            api_key: "".to_string(),
            model: "anthropic/claude-3.5-sonnet".to_string(),
            base_url: None,
            temperature: Some(0.7),
        },
        ollama_config: OllamaConfig {
            base_url: "http://localhost:11434".to_string(),
            model: "gemma3".to_string(),
            temperature: Some(0.7),
        },
        mcp_servers: vec![],
    }
}

/// Validate configuration
fn validate_config(config: &AppConfig) -> Result<(), String> {
    // Validate active provider config
    match config.active_provider {
        LlmProvider::OpenRouter => {
            if config.openrouter_config.api_key.is_empty() {
                return Err("OpenRouter API key is required".to_string());
            }
            if config.openrouter_config.model.is_empty() {
                return Err("OpenRouter model is required".to_string());
            }
        }
        LlmProvider::Ollama => {
            if config.ollama_config.model.is_empty() {
                return Err("Ollama model is required".to_string());
            }
            if config.ollama_config.base_url.is_empty() {
                return Err("Ollama base URL is required".to_string());
            }
        }
    }
    
    // Validate MCP server configs
    for server in &config.mcp_servers {
        if server.name.is_empty() {
            return Err("MCP server name cannot be empty".to_string());
        }
    }
    
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn get_config() -> Result<AppConfig, String> {
    load_config_from_file()
}

#[tauri::command]
#[specta::specta]
pub async fn update_config(config: AppConfig) -> Result<(), String> {
    // Validate configuration before saving
    validate_config(&config)?;
    
    // Save to file
    save_config_to_file(&config)?;
    
    println!("Config updated and saved to file");
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn get_app_status(state: State<'_, AgentState>) -> Result<AppStatus, String> {
    let agent_guard = state.agent.lock().await;
    let tool_registry_guard = state.tool_registry.lock().await;
    
    // Check if agent is initialized
    let agent_connected = agent_guard.is_some();
    
    // Get available tools from registry
    let available_tools: Vec<ToolDefinition> = tool_registry_guard
        .list_tools()
        .into_iter()
        .filter_map(|tool_name| {
            let description = tool_registry_guard.get_tool_description(&tool_name)?;
            let parameters = tool_registry_guard.get_tool_parameters(&tool_name)?.clone();
            
            Some(ToolDefinition {
                name: tool_name,
                description,
                parameters: parameters.to_string(),
                server: None, // Add the missing server field
            })
        })
        .collect();
    
    Ok(AppStatus {
        connection_status: ConnectionStatus {
            provider: aether_core::types::ProviderStatus {
                connected: agent_connected,
                error: None,
            },
            mcp_servers: state.get_mcp_server_statuses().await,
        },
        available_tools,
    })
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct InitializeAgentRequest {
    pub provider: LlmProvider,
    pub openrouter_config: Option<OpenRouterConfig>,
    pub ollama_config: Option<OllamaConfig>,
    pub system_prompt: Option<String>,
}

#[tauri::command]
#[specta::specta]
pub async fn initialize_agent(
    request: InitializeAgentRequest,
    state: State<'_, AgentState>,
) -> Result<(), String> {
    info!("Initialize agent called with provider: {:?}", request.provider);
    
    // Create the appropriate LLM provider
    let provider: Box<dyn aether_core::llm::provider::LlmProvider> = match request.provider {
        LlmProvider::OpenRouter => {
            let config = request.openrouter_config
                .ok_or("OpenRouter configuration required")?;
            
            if config.api_key.is_empty() {
                return Err("OpenRouter API key is required".to_string());
            }
            
            Box::new(OpenRouterProvider::new(
                config.api_key,
                config.model,
            ).map_err(|e| format!("Failed to create OpenRouter provider: {}", e))?)
        }
        LlmProvider::Ollama => {
            let config = request.ollama_config
                .ok_or("Ollama configuration required")?;
            
            Box::new(OllamaProvider::new(
                Some(config.base_url),
                config.model,
            ).map_err(|e| format!("Failed to create Ollama provider: {}", e))?)
        }
    };
    
    // Get tool registry
    let tool_registry = {
        let registry_guard = state.tool_registry.lock().await;
        registry_guard.clone()
    };
    
    // Create agent
    let agent = Agent::new(provider, tool_registry, request.system_prompt);
    
    // Store agent in state
    let mut agent_guard = state.agent.lock().await;
    *agent_guard = Some(agent);
    
    info!("Agent successfully initialized and stored in state");
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn test_provider_connection(
    provider: LlmProvider,
    openrouter_config: Option<OpenRouterConfig>,
    ollama_config: Option<OllamaConfig>,
) -> Result<bool, String> {
    // Create a test provider instance
    let test_provider: Box<dyn aether_core::llm::provider::LlmProvider> = match provider {
        LlmProvider::OpenRouter => {
            let config = openrouter_config
                .ok_or("OpenRouter configuration required")?;
            
            if config.api_key.is_empty() {
                return Err("OpenRouter API key is required".to_string());
            }
            
            Box::new(OpenRouterProvider::new(
                config.api_key,
                config.model,
            ).map_err(|e| format!("Failed to create OpenRouter provider: {}", e))?)
        }
        LlmProvider::Ollama => {
            let config = ollama_config
                .ok_or("Ollama configuration required")?;
            
            Box::new(OllamaProvider::new(
                Some(config.base_url),
                config.model,
            ).map_err(|e| format!("Failed to create Ollama provider: {}", e))?)
        }
    };
    
    // Create a simple test request
    let test_request = aether_core::llm::provider::ChatRequest {
        messages: vec![aether_core::llm::provider::ChatMessage::User {
            content: "Hello".to_string(),
        }],
        tools: vec![],
        temperature: Some(0.1),
    };
    
    // Test the connection by attempting to start a stream
    match test_provider.complete_stream_chunks(test_request).await {
        Ok(_) => Ok(true),
        Err(e) => Err(format!("Connection test failed: {}", e)),
    }
}

#[tauri::command]
#[specta::specta]
pub async fn start_mcp_server(
    server: McpServerWithId,
    state: State<'_, AgentState>,
) -> Result<(), String> {
    // Update status to connecting
    state.update_mcp_server_status(
        server.id.clone(),
        McpServerStatus {
            connected: false,
            error: None,
            tool_count: 0,
        }
    ).await;

    // Try to start the MCP server process
    match &server.config {
        McpServerConfig::Stdio { command, args, .. } => {
            match tokio::process::Command::new(command)
                .args(args)
                .spawn()
            {
        Ok(mut child) => {
            // Check if process is still running after a brief moment
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            
            match child.try_wait() {
                Ok(Some(status)) => {
                    // Process exited
                    let error_msg = format!("MCP server process exited with status: {}", status);
                    state.update_mcp_server_status(
                        server.id,
                        McpServerStatus {
                            connected: false,
                            error: Some(error_msg.clone()),
                            tool_count: 0,
                        }
                    ).await;
                    Err(error_msg)
                }
                Ok(None) => {
                    // Process is still running, mark as connected
                    state.update_mcp_server_status(
                        server.id,
                        McpServerStatus {
                            connected: true,
                            error: None,
                            tool_count: 0, // TODO: Get actual tool count from MCP server
                        }
                    ).await;
                    Ok(())
                }
                Err(e) => {
                    let error_msg = format!("Failed to check MCP server status: {}", e);
                    state.update_mcp_server_status(
                        server.id,
                        McpServerStatus {
                            connected: false,
                            error: Some(error_msg.clone()),
                            tool_count: 0,
                        }
                    ).await;
                    Err(error_msg)
                }
            }
        }
        Err(e) => {
            let error_msg = format!("Failed to start MCP server: {}", e);
            state.update_mcp_server_status(
                server.id,
                McpServerStatus {
                    connected: false,
                    error: Some(error_msg.clone()),
                    tool_count: 0,
                }
            ).await;
            Err(error_msg)
        }
            }
        }
        McpServerConfig::Http { .. } => {
            // HTTP MCP servers don't need to be "started" as a process
            let error_msg = "HTTP MCP servers are not supported for process management";
            state.update_mcp_server_status(
                server.id,
                McpServerStatus {
                    connected: false,
                    error: Some(error_msg.to_string()),
                    tool_count: 0,
                }
            ).await;
            Err(error_msg.to_string())
        }
    }
}

#[tauri::command]
#[specta::specta]
pub async fn stop_mcp_server(
    server_id: String,
    state: State<'_, AgentState>,
) -> Result<(), String> {
    // Mark server as disconnected
    state.update_mcp_server_status(
        server_id,
        McpServerStatus {
            connected: false,
            error: None,
            tool_count: 0,
        }
    ).await;

    // TODO: Implement actual process termination
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn test_mcp_server_connection(
    server: McpServerWithId,
    state: State<'_, AgentState>,
) -> Result<bool, String> {
    // Test MCP server connection by attempting to start it temporarily
    match &server.config {
        McpServerConfig::Stdio { command, args, .. } => {
            match tokio::process::Command::new(command)
                .args(args)
                .arg("--test")  // Assume servers support a test flag
                .output()
                .await
            {
        Ok(output) => {
            if output.status.success() {
                // Update status as connected for testing
                state.update_mcp_server_status(
                    server.id,
                    McpServerStatus {
                        connected: true,
                        error: None,
                        tool_count: 0,
                    }
                ).await;
                Ok(true)
            } else {
                let error_msg = String::from_utf8_lossy(&output.stderr).to_string();
                state.update_mcp_server_status(
                    server.id,
                    McpServerStatus {
                        connected: false,
                        error: Some(error_msg),
                        tool_count: 0,
                    }
                ).await;
                Ok(false)
            }
        }
        Err(e) => {
            let error_msg = format!("Failed to test MCP server: {}", e);
            state.update_mcp_server_status(
                server.id,
                McpServerStatus {
                    connected: false,
                    error: Some(error_msg.clone()),
                    tool_count: 0,
                }
            ).await;
            Err(error_msg)
        }
            }
        }
        McpServerConfig::Http { .. } => {
            // HTTP MCP servers could be tested by making HTTP requests
            let error_msg = "HTTP MCP server connection testing not yet implemented";
            state.update_mcp_server_status(
                server.id,
                McpServerStatus {
                    connected: false,
                    error: Some(error_msg.to_string()),
                    tool_count: 0,
                }
            ).await;
            Err(error_msg.to_string())
        }
    }
}

#[tauri::command]
#[specta::specta]
pub async fn refresh_mcp_server_status(
    server_id: String,
    state: State<'_, AgentState>,
) -> Result<McpServerStatus, String> {
    // Get current status
    let statuses = state.get_mcp_server_statuses().await;
    
    match statuses.get(&server_id) {
        Some(status) => Ok(status.clone()),
        None => {
            // No status found, create default disconnected status
            let default_status = McpServerStatus {
                connected: false,
                error: Some("Server not found".to_string()),
                tool_count: 0,
            };
            state.update_mcp_server_status(server_id, default_status.clone()).await;
            Ok(default_status)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::env;
    use aether_core::types::{LlmProvider, OpenRouterConfig, OllamaConfig};
    use aether_core::mcp::mcp_config::McpServerConfig;

    fn create_test_config() -> AppConfig {
        AppConfig {
            active_provider: LlmProvider::OpenRouter,
            openrouter_config: OpenRouterConfig {
                api_key: "test-api-key".to_string(),
                model: "anthropic/claude-3.5-sonnet".to_string(),
                base_url: None,
                temperature: Some(0.7),
            },
            ollama_config: OllamaConfig {
                base_url: "http://localhost:11434".to_string(),
                model: "llama2".to_string(),
                temperature: Some(0.7),
            },
            mcp_servers: vec![
                McpServerWithId {
                    id: "test-server-1".to_string(),
                    name: "Test MCP Server".to_string(),
                    enabled: true,
                    config: McpServerConfig::Stdio {
                        command: "test-command".to_string(),
                        args: vec!["arg1".to_string(), "arg2".to_string()],
                        env: HashMap::new(),
                    },
                }
            ],
        }
    }

    #[test]
    fn test_default_config() {
        let config = get_default_config();
        assert_eq!(config.active_provider, LlmProvider::OpenRouter);
        assert_eq!(config.openrouter_config.model, "anthropic/claude-3.5-sonnet");
        assert_eq!(config.ollama_config.base_url, "http://localhost:11434");
        assert!(config.mcp_servers.is_empty());
    }

    #[test]
    fn test_config_validation_valid() {
        let mut config = create_test_config();
        config.openrouter_config.api_key = "valid-key".to_string();
        
        assert!(validate_config(&config).is_ok());
    }

    #[test]
    fn test_config_validation_empty_openrouter_key() {
        let mut config = create_test_config();
        config.openrouter_config.api_key = "".to_string();
        
        let result = validate_config(&config);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("OpenRouter API key is required"));
    }

    #[test]
    fn test_config_validation_empty_model() {
        let mut config = create_test_config();
        config.openrouter_config.model = "".to_string();
        config.openrouter_config.api_key = "valid-key".to_string();
        
        let result = validate_config(&config);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("OpenRouter model is required"));
    }

    #[test]
    fn test_config_validation_ollama() {
        let mut config = create_test_config();
        config.active_provider = LlmProvider::Ollama;
        config.ollama_config.model = "".to_string();
        
        let result = validate_config(&config);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Ollama model is required"));
    }

    #[test]
    fn test_config_validation_empty_mcp_server_name() {
        let mut config = create_test_config();
        config.openrouter_config.api_key = "valid-key".to_string();
        config.mcp_servers[0].name = "".to_string();
        
        let result = validate_config(&config);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("MCP server name cannot be empty"));
    }

    #[test]
    fn test_env_var_substitution_simple() {
        env::set_var("TEST_VAR", "test_value");
        let input = "api_key: $TEST_VAR";
        let result = substitute_env_vars(input);
        assert_eq!(result, "api_key: test_value");
        env::remove_var("TEST_VAR");
    }

    #[test]
    fn test_env_var_substitution_missing_var() {
        let input = "api_key: $MISSING_VAR";
        let result = substitute_env_vars(input);
        assert_eq!(result, "api_key: $MISSING_VAR");
    }

    #[test]
    fn test_env_var_substitution_multiple() {
        env::set_var("VAR1", "value1");
        env::set_var("VAR2", "value2");
        let input = "key1: $VAR1, key2: $VAR2";
        let result = substitute_env_vars(input);
        assert_eq!(result, "key1: value1, key2: value2");
        env::remove_var("VAR1");
        env::remove_var("VAR2");
    }

    #[test]
    fn test_env_var_substitution_no_vars() {
        let input = "no variables here";
        let result = substitute_env_vars(input);
        assert_eq!(result, "no variables here");
    }

    #[test]
    fn test_config_dir_path() {
        let config_dir = get_config_dir();
        assert!(config_dir.is_ok());
        let path = config_dir.unwrap();
        assert!(path.to_string_lossy().contains("aether"));
    }

    #[test]
    fn test_config_file_path() {
        let config_file = get_config_file_path();
        assert!(config_file.is_ok());
        let path = config_file.unwrap();
        assert!(path.to_string_lossy().ends_with("config.json"));
    }
}