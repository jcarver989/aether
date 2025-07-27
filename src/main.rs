use clap::Parser;
use cli::Cli;
use color_eyre::Result;

use crate::app::App;

mod action;
mod app;
mod cli;
mod components;
mod config;
mod errors;
mod llm;
mod logging;
mod mcp;
mod mcp_config;
mod theme;
mod tui;
mod types;

#[tokio::main]
async fn main() -> Result<()> {
    crate::errors::init()?;
    crate::logging::init()?;

    let args = Cli::parse();
    let mut app = App::new(&args)?;
    app.run().await?;
    Ok(())
}
