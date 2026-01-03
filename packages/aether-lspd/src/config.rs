use crate::protocol::LanguageId;
use std::collections::HashMap;
use std::sync::OnceLock;

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

/// Static storage for the config map
static CONFIG_MAP: OnceLock<HashMap<LanguageId, LspConfig>> = OnceLock::new();

/// Get the configuration for a given language
///
/// Returns None if no LSP is configured for the language.
pub fn get_config_for_language(language: LanguageId) -> Option<&'static LspConfig> {
    let map = CONFIG_MAP.get_or_init(|| {
        let mut map = HashMap::new();
        for config in default_lsp_configs() {
            for lang in &config.languages {
                map.insert(*lang, config.clone());
            }
        }
        map
    });
    map.get(&language)
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
    fn test_default_lsp_configs() {
        let configs = default_lsp_configs();

        assert!(!configs.is_empty());

        let rust_config = configs.iter().find(|c| c.command == "rust-analyzer");
        assert!(rust_config.is_some());
        let rust_config = rust_config.unwrap();
        assert!(rust_config.languages.contains(&LanguageId::Rust));

        let ts_config = configs
            .iter()
            .find(|c| c.command == "typescript-language-server");
        assert!(ts_config.is_some());
        let ts_config = ts_config.unwrap();
        assert!(ts_config.languages.contains(&LanguageId::TypeScript));
        assert!(ts_config.languages.contains(&LanguageId::JavaScript));
    }

    #[test]
    fn test_get_config_for_language() {
        let rust_config = get_config_for_language(LanguageId::Rust);
        assert!(rust_config.is_some());
        assert_eq!(rust_config.unwrap().command, "rust-analyzer");

        let ts_config = get_config_for_language(LanguageId::TypeScript);
        assert!(ts_config.is_some());
        assert_eq!(ts_config.unwrap().command, "typescript-language-server");

        let plaintext_config = get_config_for_language(LanguageId::PlainText);
        assert!(plaintext_config.is_none());
    }
}
