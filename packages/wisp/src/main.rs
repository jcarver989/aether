mod colors;
mod ui;

use aether::agent::{Agent, AgentMessage::*, UserMessage, agent};
use aether::llm::local::DefaultModelProvider;
use clap::Parser;
use color_eyre::Report;
use crossterm::{queue, style::Stylize};
use futures::pin_mut;
use indicatif::ProgressBar;
use mcp_lexicon::AgentBuilderExt;
use std::collections::HashMap;
use std::io::Write;
use std::path::Path;
use tokio::fs;
use tokio_stream::StreamExt;

#[derive(Parser)]
#[command(name = "wisp")]
#[command(about = "A TUI for the Aether AI assistant")]
struct Cli {
    #[arg(help = "The prompt to send to the AI assistant")]
    prompt: Vec<String>,

    #[arg(short = 's', long = "system", help = "The LLM's system prompt")]
    system: Option<String>,

    #[arg(
        short = 'u',
        long = "url",
        help = "HTTP endpoint URL for the LLM provider. Defaults to http://localhost:8080 (LLama.cpp server's default port)",
        default_value = "http://localhost:8080"
    )]
    url: String,

    #[arg(short = 'k', long = "api-key", help = "API key for the LLM provider")]
    api_key: Option<String>,

    #[arg(
        short = 'm',
        long = "model",
        help = "Model name to use",
        default_value = ""
    )]
    model: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut stdout = std::io::stdout();
    let cli = Cli::parse();

    if cli.prompt.is_empty() {
        ui::show_usage("wisp")?;
        return Ok(());
    }

    let user_prompt = cli.prompt.join(" ");

    ui::show_wisp_logo()?;
    let (mut agent, agents_status) = build_agent(&cli).await?;

    let (agents_loaded, agents_error) = match agents_status {
        AgentsStatus::Loaded => (true, None),
        AgentsStatus::NotFound => (false, None),
        AgentsStatus::Error(ref e) => (false, Some(e.as_str())),
    };
    ui::show_init_header(&user_prompt, agents_loaded, agents_error)?;
    let (result_stream, _cancel_token) = agent.send(UserMessage::text(&user_prompt)).await;
    pin_mut!(result_stream);

    ui::show_response_header()?;

    let mut active_tool_calls: HashMap<String, (String, ProgressBar)> = HashMap::new();
    let mut message_started = false;

    while let Some(event) = result_stream.next().await {
        match event {
            Text {
                chunk, is_complete, ..
            } => {
                if is_complete {
                    print_styled!(stdout, "\n\n");
                    stdout.flush()?;
                    message_started = false;
                } else {
                    if let Some(filtered_chunk) = ui::filter_text_chunk(&chunk) {
                        if !message_started {
                            print_styled!(
                                stdout,
                                format!("{} ", "◈".with(colors::primary()).bold())
                            );
                            message_started = true;
                        }

                        print_styled!(stdout, filtered_chunk.with(colors::text_primary()));
                        stdout.flush()?;
                    }
                }
            }

            ToolCall {
                tool_call_id,
                name,
                result,
                is_complete,
                ..
            } => {
                if is_complete {
                    if let Some((tool_name, pb)) = active_tool_calls.get(&tool_call_id) {
                        pb.finish_and_clear();
                        ui::show_tool_completed(tool_name, result.as_deref())?;
                    }
                    active_tool_calls.remove(&tool_call_id);
                } else if !name.is_empty() {
                    print_styled_line!(stdout, "");
                    stdout.flush()?;
                    let pb = ui::create_tool_spinner(&name)?;
                    active_tool_calls.insert(tool_call_id, (name, pb));
                }
            }

            Error { message } => {
                ui::show_error(&message)?;
            }

            Cancelled { message } => {
                ui::show_cancelled(&message)?;
            }
        }
    }

    // Ensure any remaining output is flushed before showing completion
    stdout.flush()?;
    ui::show_completion()?;
    Ok(())
}

#[derive(Debug)]
enum AgentsStatus {
    Loaded,
    NotFound,
    Error(String),
}

async fn build_agent(cli: &Cli) -> Result<(Agent<DefaultModelProvider>, AgentsStatus), Report> {
    let llm = DefaultModelProvider::new(&cli.url, &cli.model, cli.api_key.clone())?;

    let (system_prompt, agents_status) = match load_agents_file().await {
        Ok(Some(content)) => (Some(content), AgentsStatus::Loaded),
        Ok(None) => (None, AgentsStatus::NotFound),
        Err(e) => (None, AgentsStatus::Error(e.to_string())),
    };

    let system = cli.system.as_ref().map(|s| s.as_str()).unwrap_or("");
    let combined_system = if let Some(agents_prompt) = system_prompt {
        if system.is_empty() {
            agents_prompt
        } else {
            format!("{}\n\n{}", system, agents_prompt)
        }
    } else {
        system.to_string()
    };

    let agent = agent(llm)
        .system(&combined_system)
        .coding_tools()
        .build()
        .await?;

    Ok((agent, agents_status))
}

async fn load_agents_file() -> Result<Option<String>, std::io::Error> {
    let agents_file = Path::new("./AGENTS.md");

    if !agents_file.exists() || !agents_file.is_file() {
        return Ok(None);
    }

    fs::read_to_string(agents_file).await.map(Some)
}
