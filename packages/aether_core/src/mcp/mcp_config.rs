use serde::{Deserialize, Serialize};
use specta::Type;
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(untagged)]
pub enum McpServerConfig {
    Http {
        url: String,
        #[serde(default)]
        headers: HashMap<String, String>,
    },

    Stdio {
        command: String,

        #[serde(default)]
        args: Vec<String>,

        #[serde(default)]
        env: HashMap<String, String>,
    },
}

impl Default for McpServerConfig {
    fn default() -> Self {
        McpServerConfig::Http {
            url: String::new(),
            headers: HashMap::new(),
        }
    }
}

impl McpServerConfig {}
