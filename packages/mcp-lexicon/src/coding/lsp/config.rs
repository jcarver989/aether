//! LSP server configuration
//!
//! This module provides configuration types for LSP servers, including
//! default configurations for common language servers.

use super::transport::LanguageId;

/// Configuration for an LSP server
#[derive(Debug, Clone)]
pub struct LspConfig {
    /// Command to spawn (e.g., "rust-analyzer", "typescript-language-server")
    pub command: String,
    /// Arguments to pass to the command
    pub args: Vec<String>,
    /// Languages this LSP handles
    pub languages: Vec<LanguageId>,
}

impl LspConfig {
    /// Create a new LSP configuration with the given command
    pub fn new(command: impl Into<String>) -> Self {
        Self {
            command: command.into(),
            args: Vec::new(),
            languages: Vec::new(),
        }
    }

    /// Add arguments to the LSP command
    pub fn with_args(mut self, args: Vec<String>) -> Self {
        self.args = args;
        self
    }

    /// Set the languages this LSP handles
    pub fn with_languages(mut self, languages: Vec<LanguageId>) -> Self {
        self.languages = languages;
        self
    }

    /// Get args as references (for passing to Command)
    pub fn args_as_refs(&self) -> Vec<&str> {
        self.args.iter().map(|s| s.as_str()).collect()
    }
}

/// Returns default LSP configurations for common languages
///
/// These defaults assume the LSP servers are installed and available in PATH.
/// The system will gracefully degrade if a server is not available.
pub fn default_lsp_configs() -> Vec<LspConfig> {
    vec![
        LspConfig::new("rust-analyzer").with_languages(vec![LanguageId::Rust]),
        LspConfig::new("typescript-language-server")
            .with_args(vec!["--stdio".to_string()])
            .with_languages(vec![
                LanguageId::TypeScript,
                LanguageId::TypeScriptReact,
                LanguageId::JavaScript,
                LanguageId::JavaScriptReact,
            ]),
        LspConfig::new("pyright-langserver")
            .with_args(vec!["--stdio".to_string()])
            .with_languages(vec![LanguageId::Python]),
        LspConfig::new("gopls").with_languages(vec![LanguageId::Go]),
        LspConfig::new("clangd").with_languages(vec![LanguageId::C, LanguageId::Cpp]),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lsp_config_builder() {
        let config = LspConfig::new("test-lsp")
            .with_args(vec!["--stdio".to_string(), "--debug".to_string()])
            .with_languages(vec![LanguageId::Rust]);

        assert_eq!(config.command, "test-lsp");
        assert_eq!(config.args, vec!["--stdio", "--debug"]);
        assert_eq!(config.languages, vec![LanguageId::Rust]);
    }

    #[test]
    fn test_args_as_refs() {
        let config = LspConfig::new("test-lsp")
            .with_args(vec!["--stdio".to_string(), "--debug".to_string()]);

        let refs = config.args_as_refs();
        assert_eq!(refs, vec!["--stdio", "--debug"]);
    }

    #[test]
    fn test_default_lsp_configs() {
        let configs = default_lsp_configs();

        // Should have configs for common languages
        assert!(!configs.is_empty());

        // Find rust-analyzer config
        let rust_config = configs.iter().find(|c| c.command == "rust-analyzer");
        assert!(rust_config.is_some());
        let rust_config = rust_config.unwrap();
        assert!(rust_config.languages.contains(&LanguageId::Rust));

        // Find typescript-language-server config
        let ts_config = configs
            .iter()
            .find(|c| c.command == "typescript-language-server");
        assert!(ts_config.is_some());
        let ts_config = ts_config.unwrap();
        assert!(ts_config.languages.contains(&LanguageId::TypeScript));
        assert!(ts_config.languages.contains(&LanguageId::JavaScript));
    }
}
