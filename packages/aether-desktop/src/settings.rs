//! Application settings for the desktop app.
//!
//! Settings are stored as JSON and include configuration for agent servers.

use crate::error::AetherDesktopError;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Configuration for an agent server.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentServerConfig {
    /// The command to run (e.g., "aether-acp", "/usr/local/bin/claude")
    pub command: String,
    /// Arguments to pass to the command
    #[serde(default)]
    pub args: Vec<String>,
    /// Environment variables to set
    #[serde(default)]
    pub env: HashMap<String, String>,
}

impl AgentServerConfig {
    /// Creates a new agent server config with just a command.
    pub fn new(command: impl Into<String>) -> Self {
        Self {
            command: command.into(),
            args: Vec::new(),
            env: HashMap::new(),
        }
    }

    /// Adds multiple arguments.
    pub fn with_args(mut self, args: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.args.extend(args.into_iter().map(|a| a.into()));
        self
    }

    /// Adds a single environment variable.
    pub fn with_env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.insert(key.into(), value.into());
        self
    }

    /// Converts to a command line string for display.
    pub fn to_command_line(&self) -> String {
        let mut parts = vec![self.command.clone()];
        parts.extend(self.args.iter().cloned());
        parts.join(" ")
    }
}

/// Application settings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Settings {
    /// Named agent server configurations.
    #[serde(default)]
    pub agent_servers: HashMap<String, AgentServerConfig>,
}

impl Settings {
    /// Returns the default settings with some preset agent servers.
    pub fn with_defaults() -> Self {
        let mut agent_servers = HashMap::new();

        // Default aether-acp configuration
        agent_servers.insert(
            "aether".to_string(),
            AgentServerConfig::new("aether-acp")
                .with_args(["--model", "anthropic:claude-sonnet-4-20250514"]),
        );

        Self { agent_servers }
    }

    /// Loads settings from a file path.
    pub fn load(path: &PathBuf) -> Result<Self, AetherDesktopError> {
        let content = std::fs::read_to_string(path)?;
        Ok(serde_json::from_str(&content)?)
    }

    /// Saves settings to a file path.
    pub fn save(&self, path: &PathBuf) -> Result<(), AetherDesktopError> {
        let content = serde_json::to_string_pretty(self)?;

        // Create parent directories if needed
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        std::fs::write(path, content)?;

        Ok(())
    }

    /// Returns the default settings file path.
    pub fn default_path() -> Option<PathBuf> {
        dirs::config_dir().map(|p| p.join("aether").join("settings.json"))
    }

    /// Loads settings from the default path, or returns defaults if not found.
    pub fn load_or_default() -> Self {
        Self::default_path()
            .and_then(|path| Self::load(&path).ok())
            .unwrap_or_else(Self::with_defaults)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serialize_settings() {
        let mut settings = Settings::default();
        settings.agent_servers.insert(
            "aether".to_string(),
            AgentServerConfig::new("aether-acp")
                .with_args(["--model", "anthropic:claude-sonnet-4-20250514"])
                .with_env("ANTHROPIC_API_KEY", "sk-test"),
        );
        settings.agent_servers.insert(
            "claude".to_string(),
            AgentServerConfig::new("claude").with_args(["--allowedTools", "computer"]),
        );

        let json = serde_json::to_string_pretty(&settings).unwrap();
        println!("{}", json);

        // Verify it can be deserialized back
        let parsed: Settings = serde_json::from_str(&json).unwrap();
        assert_eq!(settings, parsed);
    }

    #[test]
    fn test_deserialize_settings() {
        let json = r#"{
            "agent_servers": {
                "aether": {
                    "command": "aether-acp",
                    "args": ["--model", "anthropic:claude-sonnet-4-20250514"],
                    "env": {
                        "ANTHROPIC_API_KEY": "sk-test"
                    }
                },
                "claude": {
                    "command": "claude",
                    "args": []
                }
            }
        }"#;

        let settings: Settings = serde_json::from_str(json).unwrap();
        assert_eq!(settings.agent_servers.len(), 2);

        let aether = settings.agent_servers.get("aether").unwrap();
        assert_eq!(aether.command, "aether-acp");
        assert_eq!(aether.args.len(), 2);
        assert_eq!(
            aether.env.get("ANTHROPIC_API_KEY"),
            Some(&"sk-test".to_string())
        );

        let claude = settings.agent_servers.get("claude").unwrap();
        assert_eq!(claude.command, "claude");
        assert!(claude.args.is_empty());
        assert!(claude.env.is_empty());
    }

    #[test]
    fn test_to_command_line() {
        let config = AgentServerConfig::new("aether-acp")
            .with_args(["--model", "anthropic:claude-sonnet-4-20250514"]);

        assert_eq!(
            config.to_command_line(),
            "aether-acp --model anthropic:claude-sonnet-4-20250514"
        );
    }
}
