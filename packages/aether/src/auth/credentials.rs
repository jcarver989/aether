use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Credentials file storing both LLM provider and MCP server credentials
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CredentialsFile {
    #[serde(default)]
    pub providers: HashMap<String, ProviderCredential>,
    #[serde(default)]
    pub mcp_servers: HashMap<String, McpCredential>,
}

/// Credential for an LLM provider (e.g., Anthropic, OpenRouter)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ProviderCredential {
    ApiKey {
        key: String,
    },
    OAuth {
        access_token: String,
        refresh_token: String,
        /// Unix timestamp in milliseconds when the token expires
        expires_at: u64,
    },
}

impl ProviderCredential {
    pub fn api_key(key: &str) -> Self {
        Self::ApiKey {
            key: key.to_string(),
        }
    }

    pub fn oauth(access_token: String, refresh_token: String, expires_at: u64) -> Self {
        Self::OAuth {
            access_token,
            refresh_token,
            expires_at,
        }
    }
}

/// Credential for an MCP server (OAuth only)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpCredential {
    pub client_id: String,
    pub access_token: String,
    pub refresh_token: Option<String>,
    /// Unix timestamp in milliseconds when the token expires
    pub expires_at: Option<u64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn credentials_file_serializes_correctly() {
        let mut file = CredentialsFile::default();
        file.providers.insert(
            "anthropic".to_string(),
            ProviderCredential::api_key("sk-ant-test"),
        );
        file.providers.insert(
            "openrouter".to_string(),
            ProviderCredential::oauth(
                "access-token".to_string(),
                "refresh-token".to_string(),
                1703001600000, // milliseconds
            ),
        );
        file.mcp_servers.insert(
            "github-server".to_string(),
            McpCredential {
                client_id: "mcp-client".to_string(),
                access_token: "mcp-access".to_string(),
                refresh_token: None,
                expires_at: None,
            },
        );

        let json = serde_json::to_string_pretty(&file).unwrap();
        let parsed: CredentialsFile = serde_json::from_str(&json).unwrap();

        assert!(parsed.providers.contains_key("anthropic"));
        assert!(parsed.providers.contains_key("openrouter"));
        assert!(parsed.mcp_servers.contains_key("github-server"));
    }

    #[test]
    fn provider_credential_tagged_enum_works() {
        let api_key = ProviderCredential::api_key("test-key");
        let json = serde_json::to_string(&api_key).unwrap();
        assert!(json.contains(r#""type":"apikey""#));

        let oauth =
            ProviderCredential::oauth("access".to_string(), "refresh".to_string(), 1234567890000);
        let json = serde_json::to_string(&oauth).unwrap();
        assert!(json.contains(r#""type":"oauth""#));
    }
}
