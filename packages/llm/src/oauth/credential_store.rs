use async_trait::async_trait;
use keyring::Entry;
use oauth2::{AccessToken, RefreshToken, TokenResponse};
use rmcp::transport::auth::{
    AuthError, CredentialStore, OAuthTokenResponse, StoredCredentials, VendorExtraTokenFields,
};
use serde::{Deserialize, Serialize};
use std::future::Future;
use std::time::Duration;

use super::OAuthError;

const KEYCHAIN_SERVICE: &str = "aether-oauth-v1";

/// Credential for an OAuth provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthCredential {
    pub client_id: String,
    pub access_token: String,
    pub refresh_token: Option<String>,
    /// Unix timestamp in milliseconds when the token expires.
    pub expires_at: Option<u64>,
}

/// Trait for loading and saving OAuth credentials, keyed by server/provider ID.
///
/// The default implementation (`OAuthCredentialStore`) uses the OS keychain.
/// Tests can use an in-memory fake to avoid keychain popups.
pub trait OAuthCredentialStorage: Send + Sync {
    fn load_credential(
        &self,
        server_id: &str,
    ) -> impl Future<Output = Result<Option<OAuthCredential>, OAuthError>> + Send;

    fn save_credential(
        &self,
        server_id: &str,
        credential: OAuthCredential,
    ) -> impl Future<Output = Result<(), OAuthError>> + Send;

    fn has_credential(&self, server_id: &str) -> bool;
}

/// OAuth credential store that persists credentials in the OS keychain
/// and directly implements rmcp's `CredentialStore` trait.
///
/// Each server/provider ID maps to its own keychain entry.
#[derive(Clone, Default)]
pub struct OAuthCredentialStore {
    server_id: String,
}

impl OAuthCredentialStore {
    /// Create a new store for the given server/provider ID.
    pub fn new(server_id: &str) -> Self {
        Self { server_id: server_id.to_string() }
    }

    /// Load the raw `OAuthCredential` for this store's server ID.
    pub async fn load_credential(&self) -> Result<Option<OAuthCredential>, OAuthError> {
        let store = self.clone();
        spawn_blocking(move || store.load_sync()).await
    }

    /// Save a raw `OAuthCredential` directly, keyed by this store's server ID.
    pub async fn save_credential(&self, credential: OAuthCredential) -> Result<(), OAuthError> {
        let store = self.clone();
        spawn_blocking(move || store.save_sync(&credential)).await
    }

    /// Check synchronously whether credentials exist for a given server ID.
    pub fn has_credential(server_id: &str) -> bool {
        keychain_entry(server_id).ok().and_then(|e| e.get_secret().ok()).is_some()
    }

    fn load_sync(&self) -> Result<Option<OAuthCredential>, OAuthError> {
        load_from_keychain(&self.server_id)
    }

    fn save_sync(&self, credential: &OAuthCredential) -> Result<(), OAuthError> {
        save_to_keychain(&self.server_id, credential)
    }

    fn delete_sync(&self) -> Result<(), OAuthError> {
        let entry = keychain_entry(&self.server_id)?;
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(err) => Err(err.into()),
        }
    }
}

impl OAuthCredentialStorage for OAuthCredentialStore {
    async fn load_credential(&self, server_id: &str) -> Result<Option<OAuthCredential>, OAuthError> {
        let server_id = server_id.to_string();
        spawn_blocking(move || load_from_keychain(&server_id)).await
    }

    async fn save_credential(&self, server_id: &str, credential: OAuthCredential) -> Result<(), OAuthError> {
        let server_id = server_id.to_string();
        spawn_blocking(move || save_to_keychain(&server_id, &credential)).await
    }

    fn has_credential(&self, server_id: &str) -> bool {
        keychain_entry(server_id).ok().and_then(|e| e.get_secret().ok()).is_some()
    }
}

#[async_trait]
impl CredentialStore for OAuthCredentialStore {
    async fn load(&self) -> Result<Option<StoredCredentials>, AuthError> {
        let cred = self.load_credential().await.map_err(|e| AuthError::InternalError(e.to_string()))?;

        Ok(cred.map(|c| {
            let token_response = build_token_response(&c);
            build_stored_credentials(&c.client_id, Some(&token_response))
        }))
    }

    async fn save(&self, credentials: StoredCredentials) -> Result<(), AuthError> {
        let token = credentials
            .token_response
            .ok_or_else(|| AuthError::InternalError("No token response to save".to_string()))?;

        let expires_at = token.expires_in().map(|duration| {
            let now_ms = u64::try_from(
                std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis(),
            )
            .unwrap_or(u64::MAX);
            let duration_ms = u64::try_from(duration.as_millis()).unwrap_or(u64::MAX);
            now_ms.saturating_add(duration_ms)
        });

        let credential = OAuthCredential {
            client_id: credentials.client_id,
            access_token: token.access_token().secret().clone(),
            refresh_token: token.refresh_token().map(|t| t.secret().clone()),
            expires_at,
        };

        self.save_credential(credential).await.map_err(|e| AuthError::InternalError(e.to_string()))
    }

    async fn clear(&self) -> Result<(), AuthError> {
        let store = self.clone();
        spawn_blocking(move || store.delete_sync()).await.map_err(|e| AuthError::InternalError(e.to_string()))
    }
}

fn keychain_entry(server_id: &str) -> Result<Entry, OAuthError> {
    Ok(Entry::new(KEYCHAIN_SERVICE, server_id)?)
}

fn load_from_keychain(server_id: &str) -> Result<Option<OAuthCredential>, OAuthError> {
    let entry = keychain_entry(server_id)?;
    match entry.get_secret() {
        Ok(blob) => serde_json::from_slice(&blob)
            .map(Some)
            .map_err(|err| OAuthError::CredentialStore(format!("invalid credential: {err}"))),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(err) => Err(err.into()),
    }
}

fn save_to_keychain(server_id: &str, credential: &OAuthCredential) -> Result<(), OAuthError> {
    let entry = keychain_entry(server_id)?;
    let blob = serde_json::to_vec(credential)
        .map_err(|err| OAuthError::CredentialStore(format!("failed to serialize credential: {err}")))?;
    entry.set_secret(&blob)?;
    Ok(())
}

async fn spawn_blocking<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, OAuthError> + Send + 'static,
) -> Result<T, OAuthError> {
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|err| OAuthError::CredentialStore(format!("credential task failed: {err}")))?
}

/// Construct a `StoredCredentials` via serde deserialization.
///
/// The upstream struct is `#[non_exhaustive]` with no constructor, so this is
/// the only way to build one from outside the crate.
fn build_stored_credentials(client_id: &str, token_response: Option<&OAuthTokenResponse>) -> StoredCredentials {
    // granted_scopes and token_received_at have #[serde(default)] so we can omit them.
    serde_json::from_value(serde_json::json!({
        "client_id": client_id,
        "token_response": token_response,
    }))
    .expect("StoredCredentials deserialization from known-good fields cannot fail")
}

fn build_token_response(cred: &OAuthCredential) -> OAuthTokenResponse {
    let mut response = OAuthTokenResponse::new(
        AccessToken::new(cred.access_token.clone()),
        oauth2::basic::BasicTokenType::Bearer,
        VendorExtraTokenFields::default(),
    );

    if let Some(ref refresh) = cred.refresh_token {
        response.set_refresh_token(Some(RefreshToken::new(refresh.clone())));
    }

    if let Some(expires_at_millis) = cred.expires_at {
        let now_millis = u64::try_from(
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis(),
        )
        .unwrap_or(u64::MAX);

        if expires_at_millis > now_millis {
            response.set_expires_in(Some(&Duration::from_millis(expires_at_millis - now_millis)));
        }
    }

    response
}
