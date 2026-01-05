use aether::auth::google::{authorize_url, exchange_code};
use aether::auth::oauth_handler::{open_browser, wait_for_callback};
use aether::auth::{FileCredentialStore, ProviderCredential};
use std::error::Error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let init = authorize_url()?;

    println!("Opening browser for Google authentication...");
    if let Err(e) = open_browser(&init.url) {
        eprintln!("Failed to open browser: {e}");
        println!("\nPlease open this URL manually:\n{}", init.url);
    }

    println!("Waiting for OAuth callback on http://localhost:8085...");
    let callback = wait_for_callback(8085).await?;

    println!("Exchanging code for tokens...");
    let tokens = exchange_code(&callback.code, &init.verifier).await?;

    let store = FileCredentialStore::new()?;
    store
        .set_provider(
            "gemini",
            ProviderCredential::oauth(tokens.access, tokens.refresh, tokens.expires),
        )
        .await?;

    println!("Credentials saved. You can now use Gemini with OAuth.");
    Ok(())
}
