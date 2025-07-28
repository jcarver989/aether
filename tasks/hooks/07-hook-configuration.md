# Task 07: Hook Configuration System

## Overview
Implement configuration loading and management for hooks, allowing users to customize which tools require permissions and configure hook behavior through JSON files.

## Dependencies
- Task 02: Core Hook Infrastructure must be completed
- Task 03: Permission Hook Implementation must be completed
- Task 05: Hook Manager and Registry must be completed

## Deliverables

### 1. Hook Configuration Types (`src/hooks/config.rs`)

Create configuration structures for hooks:

```rust
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HooksConfig {
    /// Global hook settings
    pub enabled: bool,
    
    /// Default behavior for unconfigured tools
    #[serde(default = "default_behavior")]
    pub default_behavior: DefaultBehavior,
    
    /// Permission hook configuration
    pub permission: Option<PermissionHookConfig>,
    
    /// Logging hook configuration
    pub logging: Option<LoggingHookConfig>,
    
    /// Custom hook configurations
    #[serde(default)]
    pub custom: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DefaultBehavior {
    Allow,
    Deny,
    RequireApproval,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionHookConfig {
    pub enabled: bool,
    
    /// Tools that always require permission
    pub require_approval: Vec<ToolPattern>,
    
    /// Tools that are always allowed
    pub always_allow: Vec<ToolPattern>,
    
    /// Tools that are always denied
    pub always_deny: Vec<ToolPattern>,
    
    /// Custom messages for specific tools
    #[serde(default)]
    pub custom_messages: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingHookConfig {
    pub enabled: bool,
    pub log_file: Option<String>,
    pub log_level: Option<String>,
    pub include_args: bool,
    pub include_results: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ToolPattern {
    /// Exact tool name
    Exact(String),
    /// Pattern with wildcards
    Pattern { pattern: String },
    /// Advanced rule with conditions
    Rule {
        pattern: String,
        #[serde(default)]
        servers: Vec<String>,
        #[serde(default)]
        conditions: HashMap<String, serde_json::Value>,
    },
}

impl ToolPattern {
    pub fn matches(&self, tool_name: &str, server_name: &str) -> bool {
        match self {
            ToolPattern::Exact(name) => tool_name == name,
            ToolPattern::Pattern { pattern } => {
                Self::pattern_matches(pattern, tool_name)
            }
            ToolPattern::Rule { pattern, servers, .. } => {
                let pattern_match = Self::pattern_matches(pattern, tool_name);
                let server_match = servers.is_empty() || servers.contains(&server_name.to_string());
                pattern_match && server_match
            }
        }
    }
    
    fn pattern_matches(pattern: &str, text: &str) -> bool {
        if pattern.contains('*') {
            let regex_pattern = pattern.replace("*", ".*");
            regex::Regex::new(&format!("^{}$", regex_pattern))
                .map(|re| re.is_match(text))
                .unwrap_or(false)
        } else {
            pattern == text
        }
    }
}

fn default_behavior() -> DefaultBehavior {
    DefaultBehavior::RequireApproval
}

impl Default for HooksConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            default_behavior: default_behavior(),
            permission: Some(PermissionHookConfig::default()),
            logging: None,
            custom: HashMap::new(),
        }
    }
}

impl Default for PermissionHookConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            require_approval: vec![
                ToolPattern::Pattern { pattern: "write_*".to_string() },
                ToolPattern::Pattern { pattern: "delete_*".to_string() },
                ToolPattern::Pattern { pattern: "execute_*".to_string() },
                ToolPattern::Exact("run_command".to_string()),
                ToolPattern::Exact("make_http_request".to_string()),
            ],
            always_allow: vec![
                ToolPattern::Pattern { pattern: "read_*".to_string() },
                ToolPattern::Pattern { pattern: "list_*".to_string() },
                ToolPattern::Exact("get_file_info".to_string()),
            ],
            always_deny: vec![
                ToolPattern::Pattern { pattern: "sudo_*".to_string() },
                ToolPattern::Rule {
                    pattern: "*".to_string(),
                    servers: vec!["untrusted_server".to_string()],
                    conditions: HashMap::new(),
                },
            ],
            custom_messages: HashMap::new(),
        }
    }
}
```

### 2. Configuration Loading (`src/hooks/config_loader.rs`)

Implement configuration loading from files:

```rust
use super::config::{HooksConfig, PermissionHookConfig};
use color_eyre::Result;
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

pub struct HookConfigLoader {
    config_paths: Vec<PathBuf>,
}

impl HookConfigLoader {
    pub fn new() -> Self {
        Self {
            config_paths: Self::default_config_paths(),
        }
    }
    
    fn default_config_paths() -> Vec<PathBuf> {
        vec![
            // User config
            dirs::config_dir()
                .map(|d| d.join("aether").join("hooks.json"))
                .unwrap_or_default(),
            // Project config
            PathBuf::from(".aether").join("hooks.json"),
            // Fallback
            PathBuf::from("hooks.json"),
        ]
    }
    
    pub fn load(&self) -> Result<HooksConfig> {
        // Try to load from each path in order
        for path in &self.config_paths {
            if path.exists() {
                info!("Loading hooks config from: {:?}", path);
                match self.load_from_file(path) {
                    Ok(config) => return Ok(config),
                    Err(e) => {
                        warn!("Failed to load hooks config from {:?}: {}", path, e);
                    }
                }
            }
        }
        
        // If no config found, use defaults
        info!("No hooks config found, using defaults");
        Ok(HooksConfig::default())
    }
    
    fn load_from_file(&self, path: &Path) -> Result<HooksConfig> {
        let content = std::fs::read_to_string(path)?;
        let config: HooksConfig = serde_json::from_str(&content)?;
        
        // Validate config
        self.validate_config(&config)?;
        
        Ok(config)
    }
    
    fn validate_config(&self, config: &HooksConfig) -> Result<()> {
        // Check for conflicting rules
        if let Some(permission) = &config.permission {
            for pattern in &permission.require_approval {
                for allow_pattern in &permission.always_allow {
                    if self.patterns_overlap(pattern, allow_pattern) {
                        warn!(
                            "Potential conflict: pattern in require_approval overlaps with always_allow"
                        );
                    }
                }
            }
        }
        
        Ok(())
    }
    
    fn patterns_overlap(&self, p1: &super::config::ToolPattern, p2: &super::config::ToolPattern) -> bool {
        // Simple overlap detection - could be enhanced
        match (p1, p2) {
            (
                super::config::ToolPattern::Pattern { pattern: pat1 },
                super::config::ToolPattern::Pattern { pattern: pat2 },
            ) => {
                pat1 == pat2 || pat1 == "*" || pat2 == "*"
            }
            _ => false,
        }
    }
    
    pub fn save(&self, config: &HooksConfig, path: Option<&Path>) -> Result<()> {
        let save_path = path
            .map(PathBuf::from)
            .unwrap_or_else(|| self.config_paths[0].clone());
        
        // Ensure parent directory exists
        if let Some(parent) = save_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        
        let content = serde_json::to_string_pretty(config)?;
        std::fs::write(&save_path, content)?;
        
        info!("Saved hooks config to: {:?}", save_path);
        Ok(())
    }
}

/// Example configuration file content
pub fn example_hooks_config() -> &'static str {
    r#"{
  "enabled": true,
  "default_behavior": "require_approval",
  "permission": {
    "enabled": true,
    "require_approval": [
      "write_file",
      "delete_file",
      { "pattern": "execute_*" },
      {
        "pattern": "*_destructive",
        "servers": [],
        "conditions": {}
      }
    ],
    "always_allow": [
      "read_file",
      "list_files",
      { "pattern": "get_*" }
    ],
    "always_deny": [
      { "pattern": "sudo_*" },
      {
        "pattern": "*",
        "servers": ["untrusted_server"],
        "conditions": {}
      }
    ],
    "custom_messages": {
      "delete_file": "This will permanently delete a file. Are you sure?",
      "execute_command": "This will run a system command. Please review carefully."
    }
  },
  "logging": {
    "enabled": true,
    "log_file": "~/.aether/tool_executions.log",
    "log_level": "info",
    "include_args": true,
    "include_results": false
  }
}"#
}
```

### 3. Update Permission Hook to Use Config

Modify `src/hooks/permission.rs` to use configuration:

```rust
use crate::hooks::config::{PermissionHookConfig, ToolPattern, DefaultBehavior};

pub struct PermissionHook {
    config: PermissionHookConfig,
    default_behavior: DefaultBehavior,
}

impl PermissionHook {
    pub fn from_config(config: PermissionHookConfig, default_behavior: DefaultBehavior) -> Self {
        Self {
            config,
            default_behavior,
        }
    }
    
    fn check_tool_permission(&self, tool_name: &str, server_name: &str) -> HookAction {
        // Check always_deny first (highest priority)
        for pattern in &self.config.always_deny {
            if pattern.matches(tool_name, server_name) {
                return HookAction::Deny(format!(
                    "Tool '{}' is in the deny list", tool_name
                ));
            }
        }
        
        // Check always_allow
        for pattern in &self.config.always_allow {
            if pattern.matches(tool_name, server_name) {
                return HookAction::Allow;
            }
        }
        
        // Check require_approval
        for pattern in &self.config.require_approval {
            if pattern.matches(tool_name, server_name) {
                return HookAction::RequireApproval;
            }
        }
        
        // Fall back to default behavior
        match self.default_behavior {
            DefaultBehavior::Allow => HookAction::Allow,
            DefaultBehavior::Deny => HookAction::Deny(
                "Tool not in allowlist".to_string()
            ),
            DefaultBehavior::RequireApproval => HookAction::RequireApproval,
        }
    }
    
    fn get_custom_message(&self, tool_name: &str) -> Option<String> {
        self.config.custom_messages.get(tool_name).cloned()
    }
}

#[async_trait]
impl Hook for PermissionHook {
    async fn pre_execute(&self, context: &HookContext) -> Result<HookResult> {
        if !self.config.enabled {
            return Ok(HookResult {
                action: HookAction::Allow,
                context: None,
            });
        }
        
        let action = self.check_tool_permission(&context.tool_name, &context.server_name);
        
        // Generate context if needed
        let result_context = match &action {
            HookAction::RequireApproval => {
                self.analyze_file_operation(&context.tool_name, &context.args).await?
            }
            _ => None,
        };
        
        Ok(HookResult {
            action,
            context: result_context,
        })
    }
    
    // ... rest of implementation
}
```

### 4. Integration with App Initialization

Show how to load and apply hook configuration:

```rust
// In main app initialization (e.g., src/app.rs or src/main.rs)

use crate::hooks::{HookConfigLoader, PermissionHook};
use std::sync::Arc;

pub async fn initialize_hooks(tool_registry: &mut ToolRegistry) -> Result<()> {
    // Load configuration
    let loader = HookConfigLoader::new();
    let config = loader.load()?;
    
    if !config.enabled {
        info!("Hooks are disabled in configuration");
        return Ok(());
    }
    
    // Register permission hook if configured
    if let Some(permission_config) = config.permission {
        let permission_hook = Arc::new(PermissionHook::from_config(
            permission_config,
            config.default_behavior,
        ));
        tool_registry.register_hook(permission_hook);
        info!("Registered permission hook");
    }
    
    // Register other hooks based on config...
    
    Ok(())
}
```

## Testing Requirements

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_tool_pattern_matching() {
        let exact = ToolPattern::Exact("write_file".to_string());
        assert!(exact.matches("write_file", "server1"));
        assert!(!exact.matches("read_file", "server1"));
        
        let pattern = ToolPattern::Pattern { 
            pattern: "write_*".to_string() 
        };
        assert!(pattern.matches("write_file", "server1"));
        assert!(pattern.matches("write_config", "server1"));
        assert!(!pattern.matches("read_file", "server1"));
        
        let rule = ToolPattern::Rule {
            pattern: "*".to_string(),
            servers: vec!["untrusted".to_string()],
            conditions: HashMap::new(),
        };
        assert!(rule.matches("any_tool", "untrusted"));
        assert!(!rule.matches("any_tool", "trusted"));
    }
    
    #[test]
    fn test_config_serialization() {
        let config = HooksConfig::default();
        let json = serde_json::to_string_pretty(&config).unwrap();
        let parsed: HooksConfig = serde_json::from_str(&json).unwrap();
        
        assert_eq!(config.enabled, parsed.enabled);
    }
    
    #[test]
    fn test_config_loading() {
        let loader = HookConfigLoader::new();
        let config = loader.load().unwrap();
        
        assert!(config.enabled);
    }
}
```

## Acceptance Criteria

- [ ] Configuration types properly serialize/deserialize to/from JSON
- [ ] Tool patterns support exact matches, wildcards, and server-specific rules
- [ ] Configuration loader tries multiple paths in order
- [ ] Invalid configurations are handled gracefully with warnings
- [ ] Permission hook uses configuration to determine behavior
- [ ] Custom messages from config are used in prompts
- [ ] Default configuration provides sensible security defaults
- [ ] Configuration can be saved back to disk
- [ ] All tests pass

## Notes for Implementation

- Use `dirs` crate for finding config directories
- Support both user-level and project-level configuration
- Configuration should be hot-reloadable in the future
- Consider adding a CLI command to generate example config
- Validate patterns don't have syntax errors
- Log configuration decisions for debugging
- Make sure sensitive patterns are in the default deny list
- Consider adding pattern priority/precedence rules