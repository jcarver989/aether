OAuth 2.0 authentication for LLM providers that require it.

This module is feature-gated behind `oauth`. Enable it in `Cargo.toml`:
```toml
aether-llm = { version = "...", features = ["oauth"] }
```

# Architecture

- [`OAuthHandler`] -- Trait for handling the OAuth callback. The handler opens a browser and waits for the authorization code on a local port.
- [`BrowserOAuthHandler`] -- Default implementation that opens the system browser and listens on a dynamic local port.
- [`OAuthCredentialStorage`] -- Trait for persisting OAuth credentials (access/refresh tokens).
- [`OAuthCredentialStore`] -- File-based implementation that stores credentials on disk.

# Running the flow

[`perform_oauth_flow`] orchestrates the full authorization code flow: browser launch, callback capture, token exchange, and credential storage.

[`create_auth_manager_from_store`] creates an auth manager from stored credentials, handling automatic token refresh.

# Errors

All OAuth-specific errors are represented by [`OAuthError`].
