use crate::auth::{AuthError, Result};
use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use rand::RngCore;
use rand::rngs::OsRng;
use reqwest::Url;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::time::{SystemTime, UNIX_EPOCH};

// Google OAuth configuration for Gemini API access
// These are from the Linear issue JOS-12
pub const CLIENT_ID: &str =
    "681255809395-oo8ft2oprdrnp9e3aqf6av3hmdib135j.apps.googleusercontent.com";
pub const CLIENT_SECRET: &str = "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";
pub const AUTH_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
pub const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
pub const REDIRECT_URI: &str = "http://localhost:8085/oauth2callback";

/// Scopes required for Gemini API access
pub const SCOPES: &[&str] = &[
    "https://www.googleapis.com/auth/cloud-platform",
    "https://www.googleapis.com/auth/userinfo.email",
    "https://www.googleapis.com/auth/userinfo.profile",
];

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

/// Generate the authorization URL for Google OAuth
pub fn authorize_url() -> Result<AuthorizeInit> {
    let verifier = generate_code_verifier();
    let challenge = code_challenge(&verifier);

    let mut url = Url::parse(AUTH_URL).map_err(|e| AuthError::Other(e.to_string()))?;
    url.query_pairs_mut()
        .append_pair("client_id", CLIENT_ID)
        .append_pair("response_type", "code")
        .append_pair("redirect_uri", REDIRECT_URI)
        .append_pair("scope", &SCOPES.join(" "))
        .append_pair("access_type", "offline") // Request refresh token
        .append_pair("prompt", "consent") // Force consent to always get refresh token
        .append_pair("code_challenge", &challenge)
        .append_pair("code_challenge_method", "S256")
        .append_pair("state", &verifier);

    Ok(AuthorizeInit {
        url: url.to_string(),
        verifier,
    })
}

/// Exchange an authorization code for tokens
pub async fn exchange_code(code: &str, verifier: &str) -> Result<OAuthTokens> {
    let request = TokenExchangeRequest {
        code,
        client_id: CLIENT_ID,
        client_secret: CLIENT_SECRET,
        redirect_uri: REDIRECT_URI,
        grant_type: "authorization_code",
        code_verifier: verifier,
    };

    let client = reqwest::Client::new();
    let response = client.post(TOKEN_URL).form(&request).send().await?;
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

/// Refresh an access token using the refresh token
pub async fn refresh(refresh_token: &str) -> Result<OAuthTokens> {
    let request = TokenRefreshRequest {
        grant_type: "refresh_token",
        refresh_token,
        client_id: CLIENT_ID,
        client_secret: CLIENT_SECRET,
    };

    let client = reqwest::Client::new();
    let response = client.post(TOKEN_URL).form(&request).send().await?;
    let status = response.status();
    let body = response.text().await?;

    if !status.is_success() {
        return Err(AuthError::Http(format!(
            "Token refresh failed with status {status}: {body}"
        )));
    }

    let parsed: TokenResponse = serde_json::from_str(&body)?;
    tokens_from_response(parsed, now_millis(), Some(refresh_token))
}

#[derive(Debug, Serialize)]
struct TokenExchangeRequest<'a> {
    code: &'a str,
    client_id: &'a str,
    client_secret: &'a str,
    redirect_uri: &'a str,
    grant_type: &'a str,
    code_verifier: &'a str,
}

#[derive(Debug, Serialize)]
struct TokenRefreshRequest<'a> {
    grant_type: &'a str,
    refresh_token: &'a str,
    client_id: &'a str,
    client_secret: &'a str,
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: Option<String>,
    expires_in: Option<u64>,
}

fn tokens_from_response(
    response: TokenResponse,
    now_ms: u64,
    fallback_refresh: Option<&str>,
) -> Result<OAuthTokens> {
    let expires = match response.expires_in {
        Some(expires_in) => now_ms.saturating_add(expires_in.saturating_mul(1000)),
        None => {
            // Default to 1 hour if not specified
            now_ms.saturating_add(3600 * 1000)
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
    OsRng.fill_bytes(&mut bytes);
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
        let init = authorize_url().expect("authorize");
        let url = Url::parse(&init.url).expect("url parse");
        assert_eq!(url.host_str(), Some("accounts.google.com"));

        let mut params = url
            .query_pairs()
            .collect::<std::collections::HashMap<_, _>>();
        assert_eq!(params.remove("client_id").as_deref(), Some(CLIENT_ID));
        assert_eq!(params.remove("response_type").as_deref(), Some("code"));
        assert_eq!(params.remove("redirect_uri").as_deref(), Some(REDIRECT_URI));
        assert_eq!(
            params.remove("scope").as_deref(),
            Some(SCOPES.join(" ").as_str())
        );
        assert_eq!(params.remove("access_type").as_deref(), Some("offline"));
        assert_eq!(params.remove("prompt").as_deref(), Some("consent"));
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
    fn tokens_from_response_uses_expires_in() {
        let response = TokenResponse {
            access_token: "access".to_string(),
            refresh_token: Some("refresh".to_string()),
            expires_in: Some(3600),
        };
        let tokens = tokens_from_response(response, 1000, None).expect("tokens");
        assert_eq!(tokens.access, "access");
        assert_eq!(tokens.refresh, "refresh");
        // 1000 + 3600*1000 = 3601000
        assert_eq!(tokens.expires, 3601000);
    }

    #[test]
    fn tokens_from_response_defaults_expires() {
        let response = TokenResponse {
            access_token: "access".to_string(),
            refresh_token: Some("refresh".to_string()),
            expires_in: None,
        };
        let tokens = tokens_from_response(response, 1000, None).expect("tokens");
        // Default is 1 hour: 1000 + 3600*1000 = 3601000
        assert_eq!(tokens.expires, 3601000);
    }

    #[test]
    fn tokens_from_response_falls_back_refresh() {
        let response = TokenResponse {
            access_token: "access".to_string(),
            refresh_token: None,
            expires_in: Some(3600),
        };
        let tokens = tokens_from_response(response, 0, Some("old_refresh")).expect("tokens");
        assert_eq!(tokens.refresh, "old_refresh");
    }

    #[test]
    fn tokens_from_response_errors_without_refresh() {
        let response = TokenResponse {
            access_token: "access".to_string(),
            refresh_token: None,
            expires_in: Some(3600),
        };
        let result = tokens_from_response(response, 0, None);
        assert!(result.is_err());
    }

    #[test]
    fn code_verifier_is_valid_length() {
        let verifier = generate_code_verifier();
        // 32 bytes base64-encoded without padding = 43 characters
        assert_eq!(verifier.len(), 43);
    }

    #[test]
    fn code_challenge_is_deterministic() {
        let verifier = "test-verifier";
        let challenge1 = code_challenge(verifier);
        let challenge2 = code_challenge(verifier);
        assert_eq!(challenge1, challenge2);
    }
}
