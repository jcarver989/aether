use aether_core::agent::{AgentMessage::*, UserMessage, agent};
use aether_core::llm::local::LocalModelProvider;
use console::Term;
use futures::pin_mut;
use indicatif::{ProgressBar, ProgressStyle};
use owo_colors::OwoColorize;
use regex::Regex;
use std::collections::HashMap;
use std::env;
use std::path::Path;
use std::time::Duration;
use tokio::fs;
use tokio_stream::StreamExt;

// Modern futuristic color palette for 2025 terminal applications
mod colors {
    use owo_colors::Rgb;

    // Futuristic color scheme with true RGB colors
    pub fn primary() -> Rgb {
        Rgb(138, 43, 226) // Electric Violet #8A2BE2
    }
    pub fn secondary() -> Rgb {
        Rgb(0, 255, 255) // Cyan #00FFFF
    }
    pub fn accent() -> Rgb {
        Rgb(255, 215, 0) // Gold #FFD700
    }
    pub fn background() -> Rgb {
        Rgb(10, 10, 20) // Deep Space #0A0A14
    }
    pub fn surface() -> Rgb {
        Rgb(18, 18, 32) // Dark Surface #121220
    }
    pub fn text_primary() -> Rgb {
        Rgb(255, 255, 255) // White #FFFFFF
    }
    pub fn text_secondary() -> Rgb {
        Rgb(176, 176, 208) // Light Gray #B0B0D0
    }
    pub fn success() -> Rgb {
        Rgb(0, 255, 127) // Spring Green #00FF7F
    }
    pub fn warning() -> Rgb {
        Rgb(255, 165, 0) // Orange #FFA500
    }
    pub fn error() -> Rgb {
        Rgb(255, 59, 48) // Red #FF3B30
    }
    pub fn info() -> Rgb {
        Rgb(78, 205, 196) // Turquoise #4ECDC4
    }
}

// UI Helper functions for clean output formatting
mod ui {
    use super::*;

    // Filter out XML tool call chunks and other implementation details
    pub fn filter_text_chunk(text: &str) -> Option<String> {
        // Skip empty or whitespace-only chunks
        if text.trim().is_empty() {
            return None;
        }

        // Regex patterns to filter out
        let xml_patterns = [
            r"<\?xml",             // XML declaration
            r"</?function[\s>]*",  // Complete or incomplete function tags
            r"</?parameter[\s>]*", // Complete or incomplete parameter tags
            r"</?invoke[\s>]*",    // Invoke tags
            r"<[^>]*>?",           // ANTML namespace tags
            r"</[^>]*>",
            r"<function=[^<]*$",  // Partial function tags at end of chunk
            r"<parameter=[^<]*$", // Partial parameter tags at end of chunk
            r"<[^<]*$",           // Partial antml tags at end of chunk
        ];

        let mut filtered = text.to_string();

        for pattern in &xml_patterns {
            if let Ok(regex) = Regex::new(pattern) {
                filtered = regex.replace_all(&filtered, "").to_string();
            }
        }

        // If the chunk was entirely XML noise, skip it
        if filtered.trim().is_empty() {
            return None;
        }

        // Clean up extra whitespace but preserve intentional formatting
        let cleaned = filtered
            .lines()
            .map(|line| line.trim_end()) // Remove trailing whitespace
            .collect::<Vec<_>>()
            .join("\n");

        // Preserve newlines that are meaningful for formatting
        let result = if cleaned.trim().is_empty() {
            None
        } else {
            Some(cleaned)
        };

        result
    }

    pub fn show_usage(program_name: &str) {
        println!("{}", "AETHER CODING AGENT".color(colors::primary()).bold());
        println!(
            "  {} {}",
            program_name,
            "- AI-powered coding assistant"
                .color(colors::accent())
                .italic()
        );
        println!();
        println!("{}", "Usage:".color(colors::secondary()).bold());
        println!(
            "  {} {}",
            program_name,
            "<your coding question or request>"
                .color(colors::success())
                .italic()
        );
        println!(
            "  {} {}",
            program_name,
            "\"help me implement a binary search tree\""
                .color(colors::warning())
                .italic()
        );
    }

    pub fn show_agents_loaded() {
        println!(
            "{} {}",
            "▶".color(colors::success()).bold(),
            "Loaded AGENTS.md instructions".color(colors::text_primary())
        );
    }

    pub fn show_agents_warning(error: &str) {
        eprintln!(
            "{} {}: {}",
            "⚠".color(colors::warning()).bold(),
            "Could not read AGENTS.md".color(colors::warning()),
            error.color(colors::error())
        );
    }

    pub fn show_no_agents_file() {
        println!(
            "{} {}",
            "ℹ".color(colors::info()).bold(),
            "No AGENTS.md file found in current directory".color(colors::text_secondary())
        );
    }

    pub fn show_query_header(prompt: &str) {
        println!();
        println!("{}", "─".repeat(60).color(colors::secondary()));
        println!(
            "{} {}",
            "◉".color(colors::secondary()).bold(),
            "User Input".bold().color(colors::text_primary())
        );
        println!("{}", "─".repeat(60).color(colors::secondary()));
        println!("   {}", prompt.italic().color(colors::secondary()));
        println!();
    }

    pub fn show_response_header() {
        println!("{}", "─".repeat(60).color(colors::primary()));
        println!(
            "{} {}",
            "⟨⟩".color(colors::primary()).bold(),
            "AI Response".bold().color(colors::text_primary())
        );
        println!("{}", "─".repeat(60).color(colors::primary()));
    }

    pub fn create_tool_spinner(name: &str) -> Result<ProgressBar, Box<dyn std::error::Error>> {
        let pb = ProgressBar::new_spinner();
        pb.set_style(
            ProgressStyle::default_spinner()
                .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏")
                .template(&format!(
                    "{{spinner}} {} Tool {} {{msg}}",
                    "⧗".color(colors::info()).bold(),
                    name.color(colors::info()).bold()
                ))?,
        );
        pb.set_message("running...");
        pb.enable_steady_tick(Duration::from_millis(100));
        Ok(pb)
    }

    pub fn show_tool_completed(tool_name: &str, result: Option<&str>) {
        println!(
            "{} {} {}",
            "✓".color(colors::success()).bold(),
            "Tool".bold().color(colors::text_primary()),
            tool_name.bold().color(colors::success())
        );

        if let Some(result) = result {
            // Try to parse JSON and extract meaningful content
            let display_result =
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(result) {
                    // Extract text content from common JSON structures
                    if let Some(text) = parsed.get("text").and_then(|v| v.as_str()) {
                        text.to_string()
                    } else if let Some(content) = parsed.get("content").and_then(|v| v.as_str()) {
                        content.to_string()
                    } else if let Some(message) = parsed.get("message").and_then(|v| v.as_str()) {
                        message.to_string()
                    } else {
                        // Fallback to pretty-printed JSON if we can't extract simple text
                        serde_json::to_string_pretty(&parsed).unwrap_or_else(|_| result.to_string())
                    }
                } else {
                    result.to_string()
                };

            if !display_result.trim().is_empty() {
                if display_result.len() > 200 {
                    let preview = &display_result[..197];
                    println!(
                        "   {} {}{}",
                        "Result:".dimmed(),
                        preview.bright_white(),
                        "...".dimmed()
                    );
                } else {
                    // Handle multi-line results better
                    let lines: Vec<&str> = display_result.lines().collect();
                    if lines.len() == 1 {
                        println!(
                            "   {} {}",
                            "Result:".dimmed(),
                            &display_result.bright_white()
                        );
                    } else {
                        println!("   {}", "Result:".dimmed());
                        for line in lines.iter().take(5) {
                            // Show max 5 lines
                            println!("     {}", line.bright_white());
                        }
                        if lines.len() > 5 {
                            println!("     {}", "...".dimmed());
                        }
                    }
                }
            }
        }
    }

    pub fn show_error(message: &str) {
        eprintln!(
            "{} {}",
            "✗".color(colors::error()).bold(),
            message.color(colors::error())
        );
    }

    pub fn show_cancelled(message: &str) {
        eprintln!(
            "{} {}",
            "⊘".color(colors::warning()).bold(),
            message.color(colors::warning())
        );
    }

    pub fn show_completion() {
        println!();
        println!("{}", "─".repeat(60).color(colors::accent()));
        println!(
            "{} {}",
            "◆".color(colors::accent()).bold(),
            "Analysis finished!".bold().color(colors::text_primary())
        );
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _term = Term::stdout();

    // Get user prompt from command line arguments
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        ui::show_usage(&args[0]);
        return Ok(());
    }

    let user_prompt = args[1..].join(" ");
    let llm = LocalModelProvider::llama_cpp()?;

    let mut system_prompt =
        "You are an autonomous coding agent with skills equivalent to a staff+ Rust engineer."
            .to_string();

    // Check for AGENTS.md file in the current working directory
    let agents_file = Path::new("./AGENTS.md");
    if agents_file.exists() && agents_file.is_file() {
        match fs::read_to_string(agents_file).await {
            Ok(content) => {
                ui::show_agents_loaded();
                system_prompt.push_str("\n\n# Additional Instructions from AGENTS.md\n\n");
                system_prompt.push_str(&content);
            }
            Err(e) => {
                ui::show_agents_warning(&e.to_string());
            }
        }
    } else {
        ui::show_no_agents_file();
    }

    let mut agent = agent(llm)
        .system(&system_prompt)
        .coding_tools()
        .build()
        .await?;

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
                    println!(); // Extra newline for spacing before next tool
                    message_started = false; // Reset for next message
                } else {
                    // Filter out XML tool call chunks and other noise
                    if let Some(filtered_chunk) = ui::filter_text_chunk(&chunk) {
                        // Add icon prefix for the first chunk of a message
                        if !message_started {
                            print!("{} ", "◉".color(colors::primary()).bold());
                            message_started = true;
                        }

                        // Color the text output to match our styling
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
                    // Ensure spinner starts on a new line
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
