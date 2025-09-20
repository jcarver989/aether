mod cli;
mod colors;
mod components;

use clap::Parser;

use tokio::task;
use tracing_subscriber;
mod app_state;

use crate::app_state::AppState;
use crate::cli::Cli;
use crate::components::Screen;
use iocraft::prelude::*;

pub struct AppViewState {}

impl AppViewState {
    pub fn new() -> Self {
        AppViewState {}
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing - set RUST_LOG env var to control log level
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();
    let state = AppState::from_cli(&cli).await?;

    let handle = task::spawn_blocking(async move || {
        let _ = element! {
            ContextProvider(value: Context::owned(state)) {
                Screen()
            }
        }
        .fullscreen()
        .await;
    })
    .await?;

    handle.await;

    Ok(())
}
