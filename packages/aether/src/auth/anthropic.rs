use crate::auth::{AuthError, Result};
use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use reqwest::Url;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::time::{SystemTime, UNIX_EPOCH};

pub const CLIENT_ID: &str = "9d1c250a-e61b-44d9-88ed-5944d1962f5e";
pub const REDIRECT_URI: &str = "https://console.anthropic.com/oauth/code/callback";
pub const TOKEN_URL: &str = "https://console.anthropic.com/v1/oauth/token";
pub const CREATE_KEY_URL: &str = "https://api.anthropic.com/api/oauth/claude_cli/create_api_key";
pub const SCOPE: &str = "org:create_api_key user:profile user:inference";

#[derive(Debug, Clone, Copy)]
pub enum AnthropicAuthMode {
    ProMax,
    Console,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AuthorizeInit {
    pub url: String,
    pub verifier: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OAuthTokens {
    pub access: String,
    pub refresh: String,
    pub expires: u64,
}

pub fn authorize_url(mode: AnthropicAuthMode) -> Result<AuthorizeInit> {
    let verifier = generate_code_verifier();
    let challenge = code_challenge(&verifier);
    let base = match mode {
        AnthropicAuthMode::ProMax => "https://claude.ai/oauth/authorize",
        AnthropicAuthMode::Console => "https://console.anthropic.com/oauth/authorize",
    };

    let mut url = Url::parse(base).map_err(|e| AuthError::Other(e.to_string()))?;
    url.query_pairs_mut()
        .append_pair("code", "true")
        .append_pair("client_id", CLIENT_ID)
        .append_pair("response_type", "code")
        .append_pair("redirect_uri", REDIRECT_URI)
        .append_pair("scope", SCOPE)
        .append_pair("code_challenge", &challenge)
        .append_pair("code_challenge_method", "S256")
        .append_pair("state", &verifier);

    Ok(AuthorizeInit {
        url: url.to_string(),
        verifier,
    })
}

pub async fn exchange_code(code: &str, verifier: &str) -> Result<OAuthTokens> {
    let (auth_code, state) = split_code_and_state(code, verifier);
    let request = TokenExchangeRequest {
        code: auth_code,
        state,
        grant_type: "authorization_code",
        client_id: CLIENT_ID,
        redirect_uri: REDIRECT_URI,
        code_verifier: verifier,
    };

    let client = reqwest::Client::new();
    let response = client.post(TOKEN_URL).json(&request).send().await?;
    let status = response.status();
    let body = response.text().await?;
    if !status.is_success() {
        return Err(AuthError::Http(format!(
            "Token exchange failed with status {status}: {body}"
        )));
    }

    let parsed: TokenResponse = serde_json::from_str(&body)?;
    tokens_from_response(parsed, now_millis(), None)
}

pub async fn refresh(refresh: &str) -> Result<OAuthTokens> {
    let request = TokenRefreshRequest {
        grant_type: "refresh_token",
        refresh_token: refresh,
        client_id: CLIENT_ID,
    };

    let client = reqwest::Client::new();
    let response = client.post(TOKEN_URL).json(&request).send().await?;
    let status = response.status();
    let body = response.text().await?;
    if !status.is_success() {
        return Err(AuthError::Http(format!(
            "Token refresh failed with status {status}: {body}"
        )));
    }

    let parsed: TokenResponse = serde_json::from_str(&body)?;
    tokens_from_response(parsed, now_millis(), Some(refresh))
}

pub async fn create_api_key(access: &str) -> Result<String> {
    let client = reqwest::Client::new();
    let response = client
        .post(CREATE_KEY_URL)
        .bearer_auth(access)
        .send()
        .await?;
    let status = response.status();
    let body = response.text().await?;
    if !status.is_success() {
        return Err(AuthError::Http(format!(
            "API key creation failed with status {status}: {body}"
        )));
    }

    let parsed: CreateKeyResponse = serde_json::from_str(&body)?;
    Ok(parsed.raw_key)
}

#[derive(Debug, Serialize)]
struct TokenExchangeRequest<'a> {
    code: String,
    state: String,
    grant_type: &'a str,
    client_id: &'a str,
    redirect_uri: &'a str,
    code_verifier: &'a str,
}

#[derive(Debug, Serialize)]
struct TokenRefreshRequest<'a> {
    grant_type: &'a str,
    refresh_token: &'a str,
    client_id: &'a str,
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: Option<String>,
    expires_at: Option<u64>,
    expires_in: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct CreateKeyResponse {
    raw_key: String,
}

fn split_code_and_state(code: &str, verifier: &str) -> (String, String) {
    match code.split_once('#') {
        Some((auth_code, state)) => (auth_code.to_string(), state.to_string()),
        None => (code.to_string(), verifier.to_string()),
    }
}

fn tokens_from_response(
    response: TokenResponse,
    now_ms: u64,
    fallback_refresh: Option<&str>,
) -> Result<OAuthTokens> {
    let expires = match (response.expires_at, response.expires_in) {
        (Some(expires_at), _) => expires_at,
        (None, Some(expires_in)) => now_ms.saturating_add(expires_in.saturating_mul(1000)),
        (None, None) => {
            return Err(AuthError::InvalidResponse(
                "Missing expires_at/expires_in in token response".to_string(),
            ));
        }
    };

    let refresh = match (response.refresh_token, fallback_refresh) {
        (Some(refresh), _) => refresh,
        (None, Some(fallback)) => fallback.to_string(),
        (None, None) => {
            return Err(AuthError::InvalidResponse(
                "Missing refresh_token in token response".to_string(),
            ));
        }
    };

    Ok(OAuthTokens {
        access: response.access_token,
        refresh,
        expires,
    })
}

fn generate_code_verifier() -> String {
    let mut bytes = [0u8; 32];
    rand::fill(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

fn code_challenge(verifier: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(verifier.as_bytes());
    let digest = hasher.finalize();
    URL_SAFE_NO_PAD.encode(digest)
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authorize_url_builds_expected_params() {
        let init = authorize_url(AnthropicAuthMode::ProMax).expect("authorize");
        let url = Url::parse(&init.url).expect("url parse");
        assert_eq!(url.host_str(), Some("claude.ai"));

        let mut params = url
            .query_pairs()
            .collect::<std::collections::HashMap<_, _>>();
        assert_eq!(params.remove("code").as_deref(), Some("true"));
        assert_eq!(params.remove("client_id").as_deref(), Some(CLIENT_ID));
        assert_eq!(params.remove("response_type").as_deref(), Some("code"));
        assert_eq!(params.remove("redirect_uri").as_deref(), Some(REDIRECT_URI));
        assert_eq!(params.remove("scope").as_deref(), Some(SCOPE));
        assert_eq!(
            params.remove("code_challenge_method").as_deref(),
            Some("S256")
        );
        assert_eq!(
            params.remove("state").as_deref(),
            Some(init.verifier.as_str())
        );
        let expected_challenge = code_challenge(&init.verifier);
        assert_eq!(
            params.remove("code_challenge").as_deref(),
            Some(expected_challenge.as_str())
        );
    }

    #[test]
    fn split_code_and_state_handles_fragment() {
        let (code, state) = split_code_and_state("abc123#state456", "verifier");
        assert_eq!(code, "abc123");
        assert_eq!(state, "state456");
    }

    #[test]
    fn split_code_and_state_defaults_state() {
        let (code, state) = split_code_and_state("abc123", "verifier");
        assert_eq!(code, "abc123");
        assert_eq!(state, "verifier");
    }

    #[test]
    fn tokens_from_response_prefers_expires_at() {
        let response = TokenResponse {
            access_token: "access".to_string(),
            refresh_token: Some("refresh".to_string()),
            expires_at: Some(1234),
            expires_in: Some(9),
        };
        let tokens = tokens_from_response(response, 100, None).expect("tokens");
        assert_eq!(tokens.expires, 1234);
    }

    #[test]
    fn tokens_from_response_uses_expires_in() {
        let response = TokenResponse {
            access_token: "access".to_string(),
            refresh_token: Some("refresh".to_string()),
            expires_at: None,
            expires_in: Some(2),
        };
        let tokens = tokens_from_response(response, 1000, None).expect("tokens");
        assert_eq!(tokens.expires, 3000);
    }

    #[test]
    fn tokens_from_response_falls_back_refresh() {
        let response = TokenResponse {
            access_token: "access".to_string(),
            refresh_token: None,
            expires_at: Some(1234),
            expires_in: None,
        };
        let tokens = tokens_from_response(response, 0, Some("old")).expect("tokens");
        assert_eq!(tokens.refresh, "old");
    }
}
