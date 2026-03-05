use clap::Args;

#[derive(Args)]
pub struct AuthArgs {
    /// Provider to authenticate (e.g. "codex")
    provider: String,
}

pub async fn run_auth(args: AuthArgs) -> Result<(), String> {
    match args.provider.as_str() {
        "codex" => {
            println!("Opening browser for ChatGPT authentication...");
            llm::perform_codex_oauth_flow()
                .await
                .map_err(|e| format!("Authentication failed: {e}"))?;
            println!("Authentication successful! You can now use Codex models.");
            Ok(())
        }
        other => Err(format!("Unknown provider: {other}. Available: codex")),
    }
}
