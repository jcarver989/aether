use aether::auth::store;
use aether::auth::{
    AnthropicAuthMode, ProviderCredentials, authorize_url, create_api_key, exchange_code,
};
use clap::Parser;
use std::error::Error;
use std::io::{self, Write};

#[derive(Parser)]
#[command(
    author,
    version,
    about = "Run Anthropic OAuth flow and save credentials"
)]
struct Args {
    /// OAuth mode: "pro" (claude.ai) or "console" (console.anthropic.com)
    #[arg(long, default_value = "console")]
    mode: String,

    /// Create and store an API key instead of OAuth tokens
    #[arg(long)]
    create_api_key: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();
    let mode = match args.mode.as_str() {
        "pro" | "promax" => AnthropicAuthMode::ProMax,
        "console" => AnthropicAuthMode::Console,
        other => {
            eprintln!("Unknown mode: {other}. Use \"pro\" or \"console\".");
            std::process::exit(2);
        }
    };

    let init = authorize_url(mode)?;
    println!("Open this URL in your browser:\n{}", init.url);
    println!("Paste the full code from the callback (include #state if present).");
    print!("Code: ");
    io::stdout().flush()?;

    let mut code = String::new();
    io::stdin().read_line(&mut code)?;
    let code = code.trim();

    let tokens = exchange_code(code, &init.verifier).await?;

    let mut store_data = store::load()?;
    if args.create_api_key {
        let api_key = create_api_key(&tokens.access).await?;
        store_data.providers.insert(
            "anthropic".to_string(),
            ProviderCredentials::api_key(&api_key),
        );
        store::save(&store_data)?;
        println!("Saved Anthropic API key to {}", store::path()?.display());
        return Ok(());
    }

    store_data.providers.insert(
        "anthropic".to_string(),
        ProviderCredentials::OAuth {
            access: tokens.access,
            refresh: tokens.refresh,
            expires: tokens.expires,
        },
    );
    store::save(&store_data)?;
    println!(
        "Saved Anthropic OAuth tokens to {}",
        store::path()?.display()
    );

    Ok(())
}
