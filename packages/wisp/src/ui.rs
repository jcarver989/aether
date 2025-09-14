use crate::colors;
use crossterm::{
    queue,
    style::{PrintStyledContent, Stylize},
};
use indicatif::{ProgressBar, ProgressStyle};
use regex::Regex;
use std::io::{Write, stdout};
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

pub fn show_wisp_logo() -> Result<(), std::io::Error> {
    let mut stdout = stdout();
    // Load and display the pre-colored logo from logo.txt
    let logo_path = std::path::Path::new("src/logo.txt");
    if let Ok(logo_content) = std::fs::read_to_string(logo_path) {
        queue!(stdout, PrintStyledContent("\n".stylize()))?;
        queue!(stdout, PrintStyledContent(logo_content.stylize()))?;
        queue!(stdout, PrintStyledContent("\n".stylize()))?;
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
            queue!(stdout, PrintStyledContent(padding.clone().stylize()))?;
            let chars: Vec<char> = line.chars().collect();

            for (_i, ch) in chars.iter().enumerate() {
                queue!(
                    stdout,
                    PrintStyledContent(ch.to_string().with(colors::primary()).bold())
                )?;
            }
            queue!(stdout, PrintStyledContent("\n".stylize()))?;
        }
        queue!(stdout, PrintStyledContent("\n".stylize()))?;
        let tagline_padding = " ".repeat(20); // Center "Ethereal AI Assistant" (24 chars): (128-24)/2 = 52, but adjust for visual balance
        queue!(
            stdout,
            PrintStyledContent(
                format!(
                    "{}{}",
                    tagline_padding,
                    "Ethereal AI Assistant".dim().italic()
                )
                .stylize()
            )
        )?;
        queue!(stdout, PrintStyledContent("\n\n".stylize()))?;
    } else {
        // Fallback to simple text logo if file not found
        queue!(stdout, PrintStyledContent("\n".stylize()))?;
        queue!(
            stdout,
            PrintStyledContent(
                format!("           {}", "W I S P".with(colors::primary()).bold()).stylize()
            )
        )?;
        queue!(stdout, PrintStyledContent("\n".stylize()))?;
        queue!(
            stdout,
            PrintStyledContent(
                format!(
                    "           {}",
                    "Ethereal AI Assistant".with(colors::info()).italic()
                )
                .stylize()
            )
        )?;
        queue!(stdout, PrintStyledContent("\n\n".stylize()))?;
    }
    stdout.flush()?;
    Ok(())
}

pub fn show_usage(program_name: &str) -> Result<(), std::io::Error> {
    show_wisp_logo()?;
    let mut stdout = stdout();
    queue!(
        stdout,
        PrintStyledContent("Usage:".with(colors::secondary()).bold())
    )?;
    queue!(stdout, PrintStyledContent("\n".stylize()))?;
    queue!(
        stdout,
        PrintStyledContent(
            format!(
                "  {} {}",
                program_name,
                "<your coding question or request>"
                    .with(colors::success())
                    .italic()
            )
            .stylize()
        )
    )?;
    queue!(stdout, PrintStyledContent("\n".stylize()))?;
    queue!(
        stdout,
        PrintStyledContent(
            format!(
                "  {} {}",
                program_name,
                "\"help me implement a binary search tree\""
                    .with(colors::warning())
                    .italic()
            )
            .stylize()
        )
    )?;
    queue!(stdout, PrintStyledContent("\n".stylize()))?;
    stdout.flush()?;
    Ok(())
}

pub fn show_init_header(
    prompt: &str,
    agents_loaded: bool,
    agents_error: Option<&str>,
) -> Result<(), std::io::Error> {
    let mut stdout = stdout();
    queue!(stdout, PrintStyledContent("\n".stylize()))?;
    queue!(
        stdout,
        PrintStyledContent("─".repeat(60).with(colors::info()))
    )?;
    queue!(stdout, PrintStyledContent("\n".stylize()))?;
    queue!(
        stdout,
        PrintStyledContent(
            format!(
                "{} {}",
                "⚙".with(colors::info()).bold(),
                "Init".bold().with(colors::text_primary())
            )
            .stylize()
        )
    )?;
    queue!(stdout, PrintStyledContent("\n".stylize()))?;
    queue!(
        stdout,
        PrintStyledContent("─".repeat(60).with(colors::info()))
    )?;
    queue!(stdout, PrintStyledContent("\n\n".stylize()))?;

    // User prompt
    queue!(
        stdout,
        PrintStyledContent(
            format!(
                "  {} {}",
                "◆".with(colors::secondary()).bold(),
                "User Prompt:".bold().with(colors::text_primary())
            )
            .stylize()
        )
    )?;
    queue!(stdout, PrintStyledContent("\n".stylize()))?;
    queue!(
        stdout,
        PrintStyledContent(
            format!("    {}", prompt.italic().with(colors::text_primary())).stylize()
        )
    )?;
    queue!(stdout, PrintStyledContent("\n\n".stylize()))?;

    // Agents status
    if agents_loaded {
        queue!(
            stdout,
            PrintStyledContent(
                format!(
                    "  {} {}",
                    "✓".with(colors::success()).bold(),
                    "Loaded AGENTS.md as system prompt".with(colors::text_primary())
                )
                .stylize()
            )
        )?;
    } else if let Some(error) = agents_error {
        queue!(
            stdout,
            PrintStyledContent(
                format!(
                    "  {} {}: {}",
                    "⚠".with(colors::warning()).bold(),
                    "Could not read AGENTS.md".with(colors::warning()),
                    error.with(colors::error())
                )
                .stylize()
            )
        )?;
    } else {
        queue!(
            stdout,
            PrintStyledContent(
                format!(
                    "  {} {}",
                    "ℹ".with(colors::info()).bold(),
                    "No AGENTS.md file found in current directory".with(colors::text_secondary())
                )
                .stylize()
            )
        )?;
    }
    queue!(stdout, PrintStyledContent("\n".stylize()))?;

    queue!(
        stdout,
        PrintStyledContent("─".repeat(60).with(colors::info()))
    )?;
    queue!(stdout, PrintStyledContent("\n\n".stylize()))?;
    stdout.flush()?;
    Ok(())
}

pub fn show_response_header() -> Result<(), std::io::Error> {
    let mut stdout = stdout();
    queue!(
        stdout,
        PrintStyledContent("─".repeat(60).with(colors::primary()))
    )?;
    queue!(stdout, PrintStyledContent("\n".stylize()))?;
    queue!(
        stdout,
        PrintStyledContent(
            format!(
                "{} {}",
                "⟨⟩".with(colors::primary()).bold(),
                "Wisp's Response".bold().with(colors::text_primary())
            )
            .stylize()
        )
    )?;
    queue!(stdout, PrintStyledContent("\n".stylize()))?;
    queue!(
        stdout,
        PrintStyledContent("─".repeat(60).with(colors::primary()))
    )?;
    queue!(stdout, PrintStyledContent("\n".stylize()))?;
    stdout.flush()?;
    Ok(())
}

pub fn create_tool_spinner(name: &str) -> Result<ProgressBar, Box<dyn std::error::Error>> {
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::default_spinner()
            .tick_chars("˚∘○◌◯❍◉⊙✦✧⋆✨")
            .template(&format!(
                "{{spinner:.cyan}} Tool {} {{msg}}",
                name.with(colors::info()).bold()
            ))?,
    );
    pb.set_message("running...");
    pb.enable_steady_tick(Duration::from_millis(80));
    Ok(pb)
}

pub fn show_tool_completed(tool_name: &str, result: Option<&str>) -> Result<(), std::io::Error> {
    let mut stdout = stdout();
    queue!(
        stdout,
        PrintStyledContent(
            format!(
                "{} {} {}",
                "✓".with(colors::success()).bold(),
                "Tool".bold().with(colors::text_primary()),
                tool_name.bold().with(colors::success())
            )
            .stylize()
        )
    )?;
    queue!(stdout, PrintStyledContent("\n".stylize()))?;

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
                queue!(
                    stdout,
                    PrintStyledContent(
                        format!("   {} {}{}", "Result:".dim(), preview.dim(), "...".dim())
                            .stylize()
                    )
                )?;
                queue!(stdout, PrintStyledContent("\n".stylize()))?;
            } else {
                let lines: Vec<&str> = display_result.lines().collect();
                if lines.len() == 1 {
                    queue!(
                        stdout,
                        PrintStyledContent(
                            format!("   {} {}", "Result:".dim(), &display_result.dim()).stylize()
                        )
                    )?;
                    queue!(stdout, PrintStyledContent("\n".stylize()))?;
                } else {
                    queue!(
                        stdout,
                        PrintStyledContent(format!("   {}", "Result:".dim()).stylize())
                    )?;
                    queue!(stdout, PrintStyledContent("\n".stylize()))?;
                    for line in lines.iter().take(5) {
                        queue!(
                            stdout,
                            PrintStyledContent(format!("     {}", line.dim()).stylize())
                        )?;
                        queue!(stdout, PrintStyledContent("\n".stylize()))?;
                    }
                    if lines.len() > 5 {
                        queue!(
                            stdout,
                            PrintStyledContent(format!("     {}", "...".dim()).stylize())
                        )?;
                        queue!(stdout, PrintStyledContent("\n".stylize()))?;
                    }
                }
            }
        }
    }
    stdout.flush()?;
    Ok(())
}

pub fn show_error(message: &str) -> Result<(), std::io::Error> {
    use std::io::stderr;
    let mut stderr = stderr();
    queue!(
        stderr,
        PrintStyledContent(
            format!(
                "{} {}",
                "✗".with(colors::error()).bold(),
                message.with(colors::error())
            )
            .stylize()
        )
    )?;
    queue!(stderr, PrintStyledContent("\n".stylize()))?;
    stderr.flush()?;
    Ok(())
}

pub fn show_cancelled(message: &str) -> Result<(), std::io::Error> {
    use std::io::stderr;
    let mut stderr = stderr();
    queue!(
        stderr,
        PrintStyledContent(
            format!(
                "{} {}",
                "⊘".with(colors::warning()).bold(),
                message.with(colors::warning())
            )
            .stylize()
        )
    )?;
    queue!(stderr, PrintStyledContent("\n".stylize()))?;
    stderr.flush()?;
    Ok(())
}

pub fn show_completion() -> Result<(), std::io::Error> {
    let mut stdout = stdout();
    queue!(stdout, PrintStyledContent("\n".stylize()))?;
    queue!(
        stdout,
        PrintStyledContent("─".repeat(60).with(colors::accent()))
    )?;
    queue!(stdout, PrintStyledContent("\n".stylize()))?;
    queue!(
        stdout,
        PrintStyledContent(
            format!(
                "{} {}",
                "◆".with(colors::accent()).bold(),
                "Analysis finished!".bold().with(colors::text_primary())
            )
            .stylize()
        )
    )?;
    queue!(stdout, PrintStyledContent("\n".stylize()))?;
    stdout.flush()?;
    Ok(())
}
