use serde::{Deserialize, Serialize};

/// Credential for an LLM provider (e.g., Anthropic, `OpenRouter`)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ProviderCredential {
    ApiKey { key: String },
}

impl ProviderCredential {
    pub fn api_key(key: &str) -> Self {
        Self::ApiKey {
            key: key.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_credential_tagged_enum_works() {
        let api_key = ProviderCredential::api_key("test-key");
        let json = serde_json::to_string(&api_key).unwrap();
        assert!(json.contains(r#""type":"apikey""#));

        let parsed: ProviderCredential = serde_json::from_str(&json).unwrap();
        match parsed {
            ProviderCredential::ApiKey { key } => assert_eq!(key, "test-key"),
        }
    }
}
