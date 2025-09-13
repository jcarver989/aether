mod colors;
mod ui;

use aether::agent::{Agent, AgentMessage::*, UserMessage, agent};
use aether::llm::local::LocalModelProvider;
use color_eyre::Report;
use console::Term;
use futures::pin_mut;
use indicatif::ProgressBar;
use mcp_lexicon::AgentBuilderExt;
use owo_colors::OwoColorize;
use std::collections::HashMap;
use std::env;
use std::path::Path;
use tokio::fs;
use tokio_stream::StreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _term = Term::stdout();

    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        ui::show_usage(&args[0]);
        return Ok(());
    }

    let user_prompt = args[1..].join(" ");

    ui::show_wisp_logo();
    let mut agent = build_agent().await?;

    ui::show_query_header(&user_prompt);
    let (result_stream, _cancel_token) = agent.send(UserMessage::text(&user_prompt)).await;
    pin_mut!(result_stream);

    ui::show_response_header();

    let mut active_tool_calls: HashMap<String, (String, ProgressBar)> = HashMap::new();
    let mut message_started = false;

    while let Some(event) = result_stream.next().await {
        match event {
            Text {
                chunk, is_complete, ..
            } => {
                if is_complete {
                    println!();
                    println!();
                    message_started = false;
                } else {
                    if let Some(filtered_chunk) = ui::filter_text_chunk(&chunk) {
                        if !message_started {
                            print!("{} ", "◉".color(colors::primary()).bold());
                            message_started = true;
                        }

                        print!("{}", filtered_chunk.color(colors::text_primary()));
                        std::io::Write::flush(&mut std::io::stdout()).unwrap();
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
                        ui::show_tool_completed(tool_name, result.as_deref());
                    }
                    active_tool_calls.remove(&tool_call_id);
                } else if !name.is_empty() {
                    println!();
                    let pb = ui::create_tool_spinner(&name)?;
                    active_tool_calls.insert(tool_call_id, (name, pb));
                }
            }

            Error { message } => {
                ui::show_error(&message);
            }

            Cancelled { message } => {
                ui::show_cancelled(&message);
            }
        }
    }

    ui::show_completion();
    Ok(())
}

async fn build_agent() -> Result<Agent<LocalModelProvider>, Report> {
    let llm = LocalModelProvider::llama_cpp()?;

    let system_prompt = match load_agents_file().await {
        Ok(Some(content)) => {
            ui::show_agents_loaded();
            Some(content)
        }

        Ok(None) => {
            ui::show_no_agents_file();
            None
        }

        Err(e) => {
            ui::show_agents_warning(&e.to_string());
            None
        }
    };

    let agent = agent(llm)
        .system(&system_prompt.unwrap_or("".to_string()))
        .coding_tools()
        .build()
        .await;

    agent
}

async fn load_agents_file() -> Result<Option<String>, std::io::Error> {
    let agents_file = Path::new("./AGENTS.md");

    if !agents_file.exists() || !agents_file.is_file() {
        return Ok(None);
    }

    fs::read_to_string(agents_file).await.map(Some)
}
