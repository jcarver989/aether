use std::{
    collections::HashMap,
    env,
    path::{Path, PathBuf},
};

use color_eyre::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use derive_deref::{Deref, DerefMut};
use directories::ProjectDirs;
use lazy_static::lazy_static;
use ratatui::style::{Color, Modifier, Style};
use serde::{Deserialize, Serialize, de::Deserializer};
use tracing::error;

use crate::{action::Action, app::Mode, cli::Cli};

const CONFIG: &str = include_str!("../.config/config.json5");

#[derive(Clone, Debug, Deserialize, Default)]
pub struct AppConfig {
    #[serde(default)]
    #[allow(dead_code)]
    pub data_dir: PathBuf,
    #[serde(default)]
    #[allow(dead_code)]
    pub config_dir: PathBuf,
    #[serde(flatten)]
    pub llm: LlmConfig,
    #[serde(flatten)]
    pub mcp: McpConfig,
    #[serde(default)]
    pub agent_context: Option<String>,
    #[serde(flatten)]
    pub ui: UiConfig,
}

#[derive(Clone, Debug, Deserialize, Default)]
pub struct LlmConfig {
    #[serde(default)]
    pub provider: ProviderType,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub openrouter_api_key: Option<String>,
    #[serde(default = "default_ollama_base_url")]
    pub ollama_base_url: String,
}

#[derive(Clone, Debug, Deserialize, Default)]
pub struct McpConfig {
    #[serde(default)]
    pub servers: HashMap<String, McpServerConfig>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct UiConfig {
    #[serde(default = "default_tick_rate")]
    pub tick_rate: f64,
    #[serde(default = "default_frame_rate")]
    pub frame_rate: f64,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            tick_rate: default_tick_rate(),
            frame_rate: default_frame_rate(),
        }
    }
}

pub use crate::mcp_config::McpServerConfig;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ProviderType {
    #[default]
    OpenRouter,
    Ollama,
}

fn default_ollama_base_url() -> String {
    "http://localhost:11434".to_string()
}

fn default_tick_rate() -> f64 {
    4.0
}

fn default_frame_rate() -> f64 {
    60.0
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct Config {
    #[serde(default, flatten)]
    pub config: AppConfig,
    #[serde(default)]
    pub keybindings: KeyBindings,
    #[serde(default)]
    pub styles: Styles,
}

lazy_static! {
    pub static ref PROJECT_NAME: String = env!("CARGO_CRATE_NAME").to_uppercase().to_string();
    pub static ref DATA_FOLDER: Option<PathBuf> =
        env::var(format!("{}_DATA", PROJECT_NAME.clone()))
            .ok()
            .map(PathBuf::from);
    pub static ref CONFIG_FOLDER: Option<PathBuf> =
        env::var(format!("{}_CONFIG", PROJECT_NAME.clone()))
            .ok()
            .map(PathBuf::from);
}

impl AppConfig {
    #[allow(dead_code)]
    pub fn load() -> Result<Self, config::ConfigError> {
        let config = Config::new()?;
        Ok(config.config)
    }

    pub fn validate(&self) -> Result<(), String> {
        // Validate provider type
        match self.llm.provider {
            ProviderType::OpenRouter => {
                if self.llm.openrouter_api_key.is_none() {
                    return Err(
                        "OpenRouter API key is required when using OpenRouter provider".to_string(),
                    );
                }
            }
            ProviderType::Ollama => {
                if self.llm.ollama_base_url.is_empty() {
                    return Err("Ollama base URL cannot be empty".to_string());
                }
                // Basic URL validation
                if !self.llm.ollama_base_url.starts_with("http://")
                    && !self.llm.ollama_base_url.starts_with("https://")
                {
                    return Err("Ollama base URL must be a valid HTTP/HTTPS URL".to_string());
                }
            }
        }

        // Validate model is not empty
        if self.llm.model.is_empty() {
            return Err("Model name cannot be empty".to_string());
        }

        // Validate UI configuration
        if self.ui.tick_rate <= 0.0 {
            return Err("Tick rate must be positive".to_string());
        }
        if self.ui.frame_rate <= 0.0 {
            return Err("Frame rate must be positive".to_string());
        }

        // Validate MCP server configurations
        for (name, server_config) in &self.mcp.servers {
            match server_config {
                McpServerConfig::Http { url, .. } => {
                    if url.is_empty() {
                        return Err(format!("MCP server '{name}' has empty URL"));
                    }
                    if !url.starts_with("http://") && !url.starts_with("https://") {
                        return Err(format!(
                            "MCP server '{name}' URL must be a valid HTTP/HTTPS URL"
                        ));
                    }
                }
                McpServerConfig::Process { command, .. } => {
                    if command.is_empty() {
                        return Err(format!("MCP server '{name}' has empty command"));
                    }
                }
            }
        }

        Ok(())
    }
}

impl Config {
    #[allow(dead_code)]
    pub fn new() -> Result<Self, config::ConfigError> {
        Self::with_cli_args(None)
    }

    pub fn with_cli_args(cli_args: Option<&Cli>) -> Result<Self, config::ConfigError> {
        let default_config: Config = json5::from_str(CONFIG)
            .map_err(|e| config::ConfigError::Message(format!("Failed to parse default config: {e}")))?;
        let data_dir = get_data_dir();
        let config_dir = get_config_dir();

        // Load MCP config if it exists
        let mcp_servers = if let Ok(mcp_config) = Self::load_mcp_config("mcp.json") {
            mcp_config
        } else if let Ok(mcp_config) = Self::load_mcp_config(config_dir.join("mcp.json")) {
            mcp_config
        } else {
            HashMap::new()
        };

        // Load agent context from AGENT.md if it exists
        let agent_context = Self::load_agent_context("AGENT.md")
            .or_else(|| Self::load_agent_context(config_dir.join("AGENT.md")));

        // Get provider and model from environment variables and CLI args
        let provider = Self::get_provider_from_env_and_cli(cli_args);
        let model = Self::get_model_from_env_and_cli(&provider, cli_args);
        let openrouter_api_key = env::var("OPENROUTER_API_KEY").ok();
        let ollama_base_url =
            env::var("OLLAMA_BASE_URL").unwrap_or_else(|_| default_ollama_base_url());

        // Get UI config from CLI args with defaults
        let tick_rate = cli_args
            .map(|args| args.tick_rate)
            .unwrap_or_else(default_tick_rate);
        let frame_rate = cli_args
            .map(|args| args.frame_rate)
            .unwrap_or_else(default_frame_rate);

        let mut builder = config::Config::builder()
            .set_default("data_dir", data_dir.to_str()
                .ok_or_else(|| config::ConfigError::Message("Data directory path contains invalid UTF-8".to_string()))?)?
            .set_default("config_dir", config_dir.to_str()
                .ok_or_else(|| config::ConfigError::Message("Config directory path contains invalid UTF-8".to_string()))?)?
            .set_default("model", model.as_str())?
            .set_default("ollama_base_url", ollama_base_url.as_str())?
            .set_default("tick_rate", tick_rate)?
            .set_default("frame_rate", frame_rate)?;

        // Don't set mcp_servers as default - it will come from the deserialization
        // or remain empty if not configured

        let provider_str = match provider {
            ProviderType::OpenRouter => "openrouter",
            ProviderType::Ollama => "ollama",
        };
        builder = builder.set_default("provider", provider_str)?;

        if let Some(ref api_key) = openrouter_api_key {
            builder = builder.set_default("openrouter_api_key", api_key.as_str())?;
        }

        let config_files = [
            ("config.json5", config::FileFormat::Json5),
            ("config.json", config::FileFormat::Json),
            ("config.yaml", config::FileFormat::Yaml),
            ("config.toml", config::FileFormat::Toml),
            ("config.ini", config::FileFormat::Ini),
        ];
        let mut found_config = false;
        for (file, format) in &config_files {
            let source = config::File::from(config_dir.join(file))
                .format(*format)
                .required(false);
            builder = builder.add_source(source);
            if config_dir.join(file).exists() {
                found_config = true
            }
        }
        if !found_config {
            error!("No configuration file found. Application may not behave as expected");
        }

        let mut cfg: Self = builder.build()?.try_deserialize()?;

        // Set MCP servers if they weren't loaded from config
        if cfg.config.mcp.servers.is_empty() {
            cfg.config.mcp.servers = mcp_servers;
        }

        // Set agent context if loaded
        if cfg.config.agent_context.is_none() {
            cfg.config.agent_context = agent_context;
        }

        // Override provider and model with env vars if they weren't set in config
        if cfg.config.llm.provider == ProviderType::default() {
            cfg.config.llm.provider = provider;
        }
        if cfg.config.llm.model.is_empty() {
            cfg.config.llm.model = model;
        }
        if let Some(api_key) = openrouter_api_key {
            cfg.config.llm.openrouter_api_key = Some(api_key);
        }
        cfg.config.llm.ollama_base_url = ollama_base_url;

        for (mode, default_bindings) in default_config.keybindings.iter() {
            let user_bindings = cfg.keybindings.entry(*mode).or_default();
            for (key, cmd) in default_bindings.iter() {
                user_bindings
                    .entry(key.clone())
                    .or_insert_with(|| cmd.clone());
            }
        }
        for (mode, default_styles) in default_config.styles.iter() {
            let user_styles = cfg.styles.entry(*mode).or_default();
            for (style_key, style) in default_styles.iter() {
                user_styles.entry(style_key.clone()).or_insert(*style);
            }
        }

        // Validate the configuration
        if let Err(validation_error) = cfg.config.validate() {
            error!("Configuration validation failed: {}", validation_error);
            // Don't fail hard on validation errors, just log them
        }

        Ok(cfg)
    }

    fn load_mcp_config<P: AsRef<Path>>(
        path: P,
    ) -> Result<HashMap<String, McpServerConfig>, std::io::Error> {
        let path = path.as_ref();
        if !path.exists() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "MCP config file not found",
            ));
        }

        let content = std::fs::read_to_string(path)?;
        let mcp_data: serde_json::Value = serde_json::from_str(&content)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

        // Try both "servers" and "mcpServers" keys for compatibility
        if let Some(servers) = mcp_data
            .get("mcpServers")
            .or_else(|| mcp_data.get("servers"))
        {
            serde_json::from_value(servers.clone())
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
        } else {
            Ok(HashMap::new())
        }
    }

    fn load_agent_context<P: AsRef<Path>>(path: P) -> Option<String> {
        let path = path.as_ref();
        if path.exists() {
            std::fs::read_to_string(path).ok()
        } else {
            None
        }
    }

    fn get_provider_from_env() -> ProviderType {
        let provider_str =
            env::var("DEFAULT_PROVIDER").unwrap_or_else(|_| "openrouter".to_string());
        match provider_str.to_lowercase().as_str() {
            "openrouter" => ProviderType::OpenRouter,
            "ollama" => ProviderType::Ollama,
            _ => ProviderType::OpenRouter, // Default to OpenRouter for invalid values
        }
    }

    fn get_provider_from_env_and_cli(cli_args: Option<&Cli>) -> ProviderType {
        // CLI args take precedence over environment variables
        if let Some(args) = cli_args {
            if let Some(ref provider_str) = args.provider {
                return match provider_str.to_lowercase().as_str() {
                    "openrouter" => ProviderType::OpenRouter,
                    "ollama" => ProviderType::Ollama,
                    _ => ProviderType::OpenRouter,
                };
            }
        }
        Self::get_provider_from_env()
    }

    fn get_model_from_env(provider: &ProviderType) -> String {
        env::var("DEFAULT_MODEL").unwrap_or_else(|_| match provider {
            ProviderType::OpenRouter => "qwen/qwen3-coder".to_string(),
            ProviderType::Ollama => "llama2".to_string(),
        })
    }

    fn get_model_from_env_and_cli(provider: &ProviderType, cli_args: Option<&Cli>) -> String {
        // CLI args take precedence over environment variables
        if let Some(args) = cli_args {
            if let Some(ref model) = args.model {
                return model.clone();
            }
        }
        Self::get_model_from_env(provider)
    }
}

pub fn get_data_dir() -> PathBuf {
    let directory = if let Some(s) = DATA_FOLDER.clone() {
        s
    } else if let Some(proj_dirs) = project_directory() {
        proj_dirs.data_local_dir().to_path_buf()
    } else {
        PathBuf::from(".").join(".data")
    };
    directory
}

pub fn get_config_dir() -> PathBuf {
    let directory = if let Some(s) = CONFIG_FOLDER.clone() {
        s
    } else if let Some(proj_dirs) = project_directory() {
        proj_dirs.config_local_dir().to_path_buf()
    } else {
        PathBuf::from(".").join(".config")
    };
    directory
}

fn project_directory() -> Option<ProjectDirs> {
    ProjectDirs::from("com", "kdheepak", env!("CARGO_PKG_NAME"))
}

#[derive(Clone, Debug, Default, Deref, DerefMut)]
pub struct KeyBindings(pub HashMap<Mode, HashMap<Vec<KeyEvent>, Action>>);

impl<'de> Deserialize<'de> for KeyBindings {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let parsed_map = HashMap::<Mode, HashMap<String, Action>>::deserialize(deserializer)?;

        let keybindings = parsed_map
            .into_iter()
            .map(|(mode, inner_map)| {
                let converted_inner_map = inner_map
                    .into_iter()
                    .filter_map(|(key_str, cmd)| {
                        match parse_key_sequence(&key_str) {
                            Ok(key_seq) => Some((key_seq, cmd)),
                            Err(e) => {
                                error!("Failed to parse key sequence '{}': {}", key_str, e);
                                None
                            }
                        }
                    })
                    .collect();
                (mode, converted_inner_map)
            })
            .collect();

        Ok(KeyBindings(keybindings))
    }
}

fn parse_key_event(raw: &str) -> Result<KeyEvent, String> {
    let raw_lower = raw.to_ascii_lowercase();
    let (remaining, modifiers) = extract_modifiers(&raw_lower);
    parse_key_code_with_modifiers(remaining, modifiers)
}

fn extract_modifiers(raw: &str) -> (&str, KeyModifiers) {
    let mut modifiers = KeyModifiers::empty();
    let mut current = raw;

    loop {
        match current {
            rest if rest.starts_with("ctrl-") => {
                modifiers.insert(KeyModifiers::CONTROL);
                current = &rest[5..];
            }
            rest if rest.starts_with("alt-") => {
                modifiers.insert(KeyModifiers::ALT);
                current = &rest[4..];
            }
            rest if rest.starts_with("shift-") => {
                modifiers.insert(KeyModifiers::SHIFT);
                current = &rest[6..];
            }
            _ => break, // break out of the loop if no known prefix is detected
        };
    }

    (current, modifiers)
}

fn parse_key_code_with_modifiers(
    raw: &str,
    mut modifiers: KeyModifiers,
) -> Result<KeyEvent, String> {
    let c = match raw {
        "esc" => KeyCode::Esc,
        "enter" => KeyCode::Enter,
        "left" => KeyCode::Left,
        "right" => KeyCode::Right,
        "up" => KeyCode::Up,
        "down" => KeyCode::Down,
        "home" => KeyCode::Home,
        "end" => KeyCode::End,
        "pageup" => KeyCode::PageUp,
        "pagedown" => KeyCode::PageDown,
        "backtab" => {
            modifiers.insert(KeyModifiers::SHIFT);
            KeyCode::BackTab
        }
        "backspace" => KeyCode::Backspace,
        "delete" => KeyCode::Delete,
        "insert" => KeyCode::Insert,
        "f1" => KeyCode::F(1),
        "f2" => KeyCode::F(2),
        "f3" => KeyCode::F(3),
        "f4" => KeyCode::F(4),
        "f5" => KeyCode::F(5),
        "f6" => KeyCode::F(6),
        "f7" => KeyCode::F(7),
        "f8" => KeyCode::F(8),
        "f9" => KeyCode::F(9),
        "f10" => KeyCode::F(10),
        "f11" => KeyCode::F(11),
        "f12" => KeyCode::F(12),
        "space" => KeyCode::Char(' '),
        "hyphen" => KeyCode::Char('-'),
        "minus" => KeyCode::Char('-'),
        "tab" => KeyCode::Tab,
        c if c.len() == 1 => {
            let mut c = c.chars().next()
                .ok_or_else(|| "Empty character string".to_string())?;
            if modifiers.contains(KeyModifiers::SHIFT) {
                c = c.to_ascii_uppercase();
            }
            KeyCode::Char(c)
        }
        _ => return Err(format!("Unable to parse {raw}")),
    };
    Ok(KeyEvent::new(c, modifiers))
}

#[allow(dead_code)]
pub fn key_event_to_string(key_event: &KeyEvent) -> String {
    let char;
    let key_code = match key_event.code {
        KeyCode::Backspace => "backspace",
        KeyCode::Enter => "enter",
        KeyCode::Left => "left",
        KeyCode::Right => "right",
        KeyCode::Up => "up",
        KeyCode::Down => "down",
        KeyCode::Home => "home",
        KeyCode::End => "end",
        KeyCode::PageUp => "pageup",
        KeyCode::PageDown => "pagedown",
        KeyCode::Tab => "tab",
        KeyCode::BackTab => "backtab",
        KeyCode::Delete => "delete",
        KeyCode::Insert => "insert",
        KeyCode::F(c) => {
            char = format!("f({c})");
            &char
        }
        KeyCode::Char(' ') => "space",
        KeyCode::Char(c) => {
            char = c.to_string();
            &char
        }
        KeyCode::Esc => "esc",
        KeyCode::Null => "",
        KeyCode::CapsLock => "",
        KeyCode::Menu => "",
        KeyCode::ScrollLock => "",
        KeyCode::Media(_) => "",
        KeyCode::NumLock => "",
        KeyCode::PrintScreen => "",
        KeyCode::Pause => "",
        KeyCode::KeypadBegin => "",
        KeyCode::Modifier(_) => "",
    };

    let mut modifiers = Vec::with_capacity(3);

    if key_event.modifiers.intersects(KeyModifiers::CONTROL) {
        modifiers.push("ctrl");
    }

    if key_event.modifiers.intersects(KeyModifiers::SHIFT) {
        modifiers.push("shift");
    }

    if key_event.modifiers.intersects(KeyModifiers::ALT) {
        modifiers.push("alt");
    }

    let mut key = modifiers.join("-");

    if !key.is_empty() {
        key.push('-');
    }
    key.push_str(key_code);

    key
}

pub fn parse_key_sequence(raw: &str) -> Result<Vec<KeyEvent>, String> {
    if raw.chars().filter(|c| *c == '>').count() != raw.chars().filter(|c| *c == '<').count() {
        return Err(format!("Unable to parse `{raw}`"));
    }
    let raw = if !raw.contains("><") {
        let raw = raw.strip_prefix('<').unwrap_or(raw);
        let raw = raw.strip_prefix('>').unwrap_or(raw);
        raw
    } else {
        raw
    };
    let sequences = raw
        .split("><")
        .map(|seq| {
            if let Some(s) = seq.strip_prefix('<') {
                s
            } else if let Some(s) = seq.strip_suffix('>') {
                s
            } else {
                seq
            }
        })
        .collect::<Vec<_>>();

    sequences.into_iter().map(parse_key_event).collect()
}

#[derive(Clone, Debug, Default, Deref, DerefMut)]
pub struct Styles(pub HashMap<Mode, HashMap<String, Style>>);

impl<'de> Deserialize<'de> for Styles {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let parsed_map = HashMap::<Mode, HashMap<String, String>>::deserialize(deserializer)?;

        let styles = parsed_map
            .into_iter()
            .map(|(mode, inner_map)| {
                let converted_inner_map = inner_map
                    .into_iter()
                    .map(|(str, style)| (str, parse_style(&style)))
                    .collect();
                (mode, converted_inner_map)
            })
            .collect();

        Ok(Styles(styles))
    }
}

pub fn parse_style(line: &str) -> Style {
    let (foreground, background) =
        line.split_at(line.to_lowercase().find("on ").unwrap_or(line.len()));
    let foreground = process_color_string(foreground);
    let background = process_color_string(&background.replace("on ", ""));

    let mut style = Style::default();
    if let Some(fg) = parse_color(&foreground.0) {
        style = style.fg(fg);
    }
    if let Some(bg) = parse_color(&background.0) {
        style = style.bg(bg);
    }
    style = style.add_modifier(foreground.1 | background.1);
    style
}

fn process_color_string(color_str: &str) -> (String, Modifier) {
    let color = color_str
        .replace("grey", "gray")
        .replace("bright ", "")
        .replace("bold ", "")
        .replace("underline ", "")
        .replace("inverse ", "");

    let mut modifiers = Modifier::empty();
    if color_str.contains("underline") {
        modifiers |= Modifier::UNDERLINED;
    }
    if color_str.contains("bold") {
        modifiers |= Modifier::BOLD;
    }
    if color_str.contains("inverse") {
        modifiers |= Modifier::REVERSED;
    }

    (color, modifiers)
}

fn parse_color(s: &str) -> Option<Color> {
    let s = s.trim_start();
    let s = s.trim_end();
    if s.contains("bright color") {
        let s = s.trim_start_matches("bright ");
        let c = s
            .trim_start_matches("color")
            .parse::<u8>()
            .unwrap_or_default();
        Some(Color::Indexed(c.wrapping_shl(8)))
    } else if s.contains("color") {
        let c = s
            .trim_start_matches("color")
            .parse::<u8>()
            .unwrap_or_default();
        Some(Color::Indexed(c))
    } else if s.contains("gray") {
        let c = 232
            + s.trim_start_matches("gray")
                .parse::<u8>()
                .unwrap_or_default();
        Some(Color::Indexed(c))
    } else if s.contains("rgb") {
        let red = (s.as_bytes()[3] as char).to_digit(10).unwrap_or_default() as u8;
        let green = (s.as_bytes()[4] as char).to_digit(10).unwrap_or_default() as u8;
        let blue = (s.as_bytes()[5] as char).to_digit(10).unwrap_or_default() as u8;
        let c = 16 + red * 36 + green * 6 + blue;
        Some(Color::Indexed(c))
    } else if s == "bold black" {
        Some(Color::Indexed(8))
    } else if s == "bold red" {
        Some(Color::Indexed(9))
    } else if s == "bold green" {
        Some(Color::Indexed(10))
    } else if s == "bold yellow" {
        Some(Color::Indexed(11))
    } else if s == "bold blue" {
        Some(Color::Indexed(12))
    } else if s == "bold magenta" {
        Some(Color::Indexed(13))
    } else if s == "bold cyan" {
        Some(Color::Indexed(14))
    } else if s == "bold white" {
        Some(Color::Indexed(15))
    } else if s == "black" {
        Some(Color::Indexed(0))
    } else if s == "red" {
        Some(Color::Indexed(1))
    } else if s == "green" {
        Some(Color::Indexed(2))
    } else if s == "yellow" {
        Some(Color::Indexed(3))
    } else if s == "blue" {
        Some(Color::Indexed(4))
    } else if s == "magenta" {
        Some(Color::Indexed(5))
    } else if s == "cyan" {
        Some(Color::Indexed(6))
    } else if s == "white" {
        Some(Color::Indexed(7))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn test_parse_style_default() {
        let style = parse_style("");
        assert_eq!(style, Style::default());
    }

    #[test]
    fn test_parse_style_foreground() {
        let style = parse_style("red");
        assert_eq!(style.fg, Some(Color::Indexed(1)));
    }

    #[test]
    fn test_parse_style_background() {
        let style = parse_style("on blue");
        assert_eq!(style.bg, Some(Color::Indexed(4)));
    }

    #[test]
    fn test_parse_style_modifiers() {
        let style = parse_style("underline red on blue");
        assert_eq!(style.fg, Some(Color::Indexed(1)));
        assert_eq!(style.bg, Some(Color::Indexed(4)));
    }

    #[test]
    fn test_process_color_string() {
        let (color, modifiers) = process_color_string("underline bold inverse gray");
        assert_eq!(color, "gray");
        assert!(modifiers.contains(Modifier::UNDERLINED));
        assert!(modifiers.contains(Modifier::BOLD));
        assert!(modifiers.contains(Modifier::REVERSED));
    }

    #[test]
    fn test_parse_color_rgb() {
        let color = parse_color("rgb123");
        let expected = 16 + 36 + 2 * 6 + 3;
        assert_eq!(color, Some(Color::Indexed(expected)));
    }

    #[test]
    fn test_parse_color_unknown() {
        let color = parse_color("unknown");
        assert_eq!(color, None);
    }

    #[test]
    fn test_config() -> Result<()> {
        let c = Config::new()?;
        assert_eq!(
            c.keybindings
                .get(&Mode::Home)
                .expect("Home mode should exist in config")
                .get(&parse_key_sequence("<q>").unwrap_or_default())
                .expect("Quit keybinding should exist in default config"),
            &Action::Quit
        );
        Ok(())
    }

    #[test]
    fn test_provider_type_default() {
        let provider = Config::get_provider_from_env();
        // Will be OpenRouter unless DEFAULT_PROVIDER env var is set
        assert!(matches!(
            provider,
            ProviderType::OpenRouter | ProviderType::Ollama
        ));
    }

    #[test]
    fn test_model_defaults() {
        let openrouter_model = Config::get_model_from_env(&ProviderType::OpenRouter);
        let ollama_model = Config::get_model_from_env(&ProviderType::Ollama);

        // Check that we get some model name
        assert!(!openrouter_model.is_empty());
        assert!(!ollama_model.is_empty());
    }

    #[test]
    fn test_load_mcp_config() {
        use std::fs;
        use tempfile::tempdir;

        let dir = tempdir().expect("Failed to create temp directory for test");
        let file_path = dir.path().join("test_mcp.json");
        let test_config = r#"{
            "servers": {
                "test": {
                    "url": "http://localhost:3000",
                    "headers": {}
                }
            }
        }"#;
        fs::write(&file_path, test_config).expect("Failed to write test config file");

        let result = Config::load_mcp_config(&file_path).expect("Failed to load test MCP config");
        assert_eq!(result.len(), 1);
        assert!(result.contains_key("test"));
        match &result["test"] {
            McpServerConfig::Http { url, .. } => {
                assert_eq!(url, "http://localhost:3000");
            }
            _ => panic!("Expected Http config"),
        }
    }

    #[test]
    fn test_simple_keys() {
        assert_eq!(
            parse_key_event("a").expect("Failed to parse key 'a'"),
            KeyEvent::new(KeyCode::Char('a'), KeyModifiers::empty())
        );

        assert_eq!(
            parse_key_event("enter").expect("Failed to parse key 'enter'"),
            KeyEvent::new(KeyCode::Enter, KeyModifiers::empty())
        );

        assert_eq!(
            parse_key_event("esc").expect("Failed to parse key 'esc'"),
            KeyEvent::new(KeyCode::Esc, KeyModifiers::empty())
        );
    }

    #[test]
    fn test_with_modifiers() {
        assert_eq!(
            parse_key_event("ctrl-a").expect("Failed to parse key 'ctrl-a'"),
            KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL)
        );

        assert_eq!(
            parse_key_event("alt-enter").expect("Failed to parse key 'alt-enter'"),
            KeyEvent::new(KeyCode::Enter, KeyModifiers::ALT)
        );

        assert_eq!(
            parse_key_event("shift-esc").expect("Failed to parse key 'shift-esc'"),
            KeyEvent::new(KeyCode::Esc, KeyModifiers::SHIFT)
        );
    }

    #[test]
    fn test_multiple_modifiers() {
        assert_eq!(
            parse_key_event("ctrl-alt-a").expect("Failed to parse key 'ctrl-alt-a'"),
            KeyEvent::new(
                KeyCode::Char('a'),
                KeyModifiers::CONTROL | KeyModifiers::ALT
            )
        );

        assert_eq!(
            parse_key_event("ctrl-shift-enter").expect("Failed to parse key 'ctrl-shift-enter'"),
            KeyEvent::new(KeyCode::Enter, KeyModifiers::CONTROL | KeyModifiers::SHIFT)
        );
    }

    #[test]
    fn test_reverse_multiple_modifiers() {
        assert_eq!(
            key_event_to_string(&KeyEvent::new(
                KeyCode::Char('a'),
                KeyModifiers::CONTROL | KeyModifiers::ALT
            )),
            "ctrl-alt-a".to_string()
        );
    }

    #[test]
    fn test_invalid_keys() {
        assert!(parse_key_event("invalid-key").is_err());
        assert!(parse_key_event("ctrl-invalid-key").is_err());
    }

    #[test]
    fn test_case_insensitivity() {
        assert_eq!(
            parse_key_event("CTRL-a").expect("Failed to parse key 'CTRL-a'"),
            KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL)
        );

        assert_eq!(
            parse_key_event("AlT-eNtEr").expect("Failed to parse key 'AlT-eNtEr'"),
            KeyEvent::new(KeyCode::Enter, KeyModifiers::ALT)
        );
    }
}
