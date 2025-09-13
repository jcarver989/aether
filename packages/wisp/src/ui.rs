use crate::colors;
use indicatif::{ProgressBar, ProgressStyle};
use owo_colors::OwoColorize;
use regex::Regex;
use std::time::Duration;

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

pub fn show_wisp_logo() {
    // Load and display the pre-colored logo from logo.txt
    let logo_path = std::path::Path::new("src/logo.txt");
    if let Ok(logo_content) = std::fs::read_to_string(logo_path) {
        println!();
        print!("{}", logo_content);
        println!();
        // Large ASCII art "WISP" with gradient effect - centered under 128-char logo
        let padding = " ".repeat(18); // Center 32-char WISP under 128-char logo: (128-32)/2 = 48

        #[rustfmt::skip]
        let wisp_lines = [
            "██╗    ██╗██╗███████╗██████╗ ",
            "██║    ██║██║██╔════╝██╔══██╗",
            "██║ █╗ ██║██║███████╗██████╔╝",
            "██║███╗██║██║╚════██║██╔═══╝ ",
            "╚███╔███╔╝██║███████║██║     ",
            " ╚══╝╚══╝ ╚═╝╚══════╝╚═╝     ",
        ];

        for line in wisp_lines {
            print!("{}", padding);
            let chars: Vec<char> = line.chars().collect();

            for (_i, ch) in chars.iter().enumerate() {
                print!("{}", ch.to_string().color(colors::primary()).bold());
            }
            println!();
        }
        println!();
        let tagline_padding = " ".repeat(20); // Center "Ethereal AI Assistant" (24 chars): (128-24)/2 = 52, but adjust for visual balance
        println!(
            "{}{}",
            tagline_padding,
            "Ethereal AI Assistant".dimmed().italic()
        );
        println!();
    } else {
        // Fallback to simple text logo if file not found
        println!();
        println!("           {}", "W I S P".color(colors::primary()).bold());
        println!(
            "           {}",
            "Ethereal AI Assistant".color(colors::info()).italic()
        );
        println!();
    }
}

pub fn show_usage(program_name: &str) {
    show_wisp_logo();
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

pub fn show_init_header(prompt: &str, agents_loaded: bool, agents_error: Option<&str>) {
    println!();
    println!("{}", "─".repeat(60).color(colors::info()));
    println!(
        "{} {}",
        "⚙".color(colors::info()).bold(),
        "Init".bold().color(colors::text_primary())
    );
    println!("{}", "─".repeat(60).color(colors::info()));

    // User prompt
    println!(
        "  {} {}",
        "◆".color(colors::secondary()).bold(),
        "User Prompt:".bold().color(colors::text_primary())
    );
    println!("    {}", prompt.italic().color(colors::text_primary()));
    println!();

    // Agents status
    if agents_loaded {
        println!(
            "  {} {}",
            "✓".color(colors::success()).bold(),
            "Loaded AGENTS.md as system prompt".color(colors::text_primary())
        );
    } else if let Some(error) = agents_error {
        println!(
            "  {} {}: {}",
            "⚠".color(colors::warning()).bold(),
            "Could not read AGENTS.md".color(colors::warning()),
            error.color(colors::error())
        );
    } else {
        println!(
            "  {} {}",
            "ℹ".color(colors::info()).bold(),
            "No AGENTS.md file found in current directory".color(colors::text_secondary())
        );
    }

    println!("{}", "─".repeat(60).color(colors::info()));
    println!();
}




pub fn show_response_header() {
    println!("{}", "─".repeat(60).color(colors::primary()));
    println!(
        "{} {}",
        "⟨⟩".color(colors::primary()).bold(),
        "Wisp's Response".bold().color(colors::text_primary())
    );
    println!("{}", "─".repeat(60).color(colors::primary()));
}

pub fn create_tool_spinner(name: &str) -> Result<ProgressBar, Box<dyn std::error::Error>> {
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::default_spinner()
            .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏")
            .template(&format!(
                "{{spinner}} Tool {} {{msg}}",
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
        let display_result = if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(result) {
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
