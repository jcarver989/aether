use crate::colors;
use crossterm::{queue, style::Stylize};
use indicatif::{ProgressBar, ProgressStyle};
use regex::Regex;
use std::io::{Write, stderr, stdout};
use std::time::Duration;

#[macro_export]
macro_rules! print_styled {
    ($writer:expr, $content:expr) => {
        queue!(
            $writer,
            crossterm::style::PrintStyledContent($content.stylize())
        )?
    };
}

#[macro_export]
macro_rules! print_styled_line {
    ($writer:expr, $content:expr) => {
        queue!(
            $writer,
            crossterm::style::PrintStyledContent($content.stylize())
        )?;
        queue!(
            $writer,
            crossterm::style::PrintStyledContent("\n".stylize())
        )?
    };
}

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
    let logo_content = include_str!("logo.txt");
    let padding = " ".repeat(18);

    #[rustfmt::skip]
        let wisp_lines = [
            "██╗    ██╗██╗███████╗██████╗ ",
            "██║    ██║██║██╔════╝██╔══██╗",
            "██║ █╗ ██║██║███████╗██████╔╝",
            "██║███╗██║██║╚════██║██╔═══╝ ",
            "╚███╔███╔╝██║███████║██║     ",
            " ╚══╝╚══╝ ╚═╝╚══════╝╚═╝     ",
        ];

    print_styled_line!(stdout, "");
    print_styled!(stdout, logo_content);
    print_styled_line!(stdout, "");

    for (line_idx, line) in wisp_lines.iter().enumerate() {
        print_styled!(stdout, padding.clone());
        let chars: Vec<char> = line.chars().collect();

        for ch in chars.iter() {
            if *ch == '█' {
                // Create vertical lighting gradient: top=full, bottom=light
                let opacity_char = match line_idx {
                    0 => '█', // Top line - full block (brightest)
                    1 => '▓', // Second line - dark shade
                    2 => '▒', // Third line - medium shade
                    3 => '▒', // Fourth line - medium shade
                    4 => '░', // Fifth line - light shade
                    _ => '░', // Bottom line - light shade (darkest)
                };
                print_styled!(
                    stdout,
                    opacity_char.to_string().with(colors::primary()).bold()
                );
            } else {
                print_styled!(stdout, ch.to_string().with(colors::primary()).bold());
            }
        }
        print_styled_line!(stdout, "");
    }
    print_styled_line!(stdout, "");
    let tagline_padding = " ".repeat(15); // Center "Ethereal AI Assistant" (24 chars): (128-24)/2 = 52, but adjust for visual balance
    print_styled!(
        stdout,
        format!(
            "{}{}",
            tagline_padding,
            "An AI agent, conjured from aether".dim().italic()
        )
    );
    print_styled!(stdout, "\n\n");

    stdout.flush()?;
    Ok(())
}

pub fn show_usage(program_name: &str) -> Result<(), std::io::Error> {
    show_wisp_logo()?;
    let mut stdout = stdout();
    print_styled_line!(stdout, "Usage:".with(colors::secondary()).bold());
    print_styled_line!(
        stdout,
        format!(
            "  {} {}",
            program_name,
            "<your coding question or request>"
                .with(colors::success())
                .italic()
        )
    );
    print_styled_line!(
        stdout,
        format!(
            "  {} {}",
            program_name,
            "\"help me implement a binary search tree\""
                .with(colors::warning())
                .italic()
        )
    );
    stdout.flush()?;
    Ok(())
}

pub fn show_init_header(
    prompt: &str,
    model_display_name: &str,
    agents_loaded: bool,
    agents_error: Option<&str>,
) -> Result<(), std::io::Error> {
    let mut stdout = stdout();
    print_styled_line!(stdout, "");
    print_styled_line!(stdout, "─".repeat(60).with(colors::info()));
    print_styled_line!(
        stdout,
        format!(
            "{} {}",
            "⚙".with(colors::info()).bold(),
            "Init".bold().with(colors::text_primary())
        )
    );
    print_styled_line!(stdout, "─".repeat(60).with(colors::info()));
    print_styled!(stdout, "\n");

    // Agents status
    if agents_loaded {
        print_styled!(
            stdout,
            format!(
                "  {} {}",
                "✓".with(colors::success()).bold(),
                format!(
                    "{} {}",
                    "System prompt:".bold().with(colors::text_primary()),
                    "loaded AGENTS.md".with(colors::text_primary())
                )
            )
        );
    } else if let Some(error) = agents_error {
        print_styled!(
            stdout,
            format!(
                "  {} {}: {}",
                "⚠".with(colors::warning()).bold(),
                "Could not read AGENTS.md".with(colors::warning()),
                error.with(colors::error())
            )
        );
    } else {
        print_styled!(
            stdout,
            format!(
                "  {} {}",
                "ℹ".with(colors::info()).bold(),
                "No AGENTS.md file found in current directory".with(colors::text_secondary())
            )
        );
    }
    print_styled!(stdout, "\n\n");

    // User prompt
    print_styled_line!(
        stdout,
        format!(
            "  {} {} {}",
            "◆".with(colors::secondary()).bold(),
            "User Prompt:".bold().with(colors::text_primary()),
            prompt.italic().with(colors::text_primary())
        )
    );
    print_styled!(stdout, "\n");

    // Model information
    print_styled_line!(
        stdout,
        format!(
            "  {} {}",
            "🤖".with(colors::primary()).bold(),
            format!(
                "{} {}",
                "Model:".bold().with(colors::text_primary()),
                model_display_name.with(colors::text_primary())
            )
        )
    );
    print_styled_line!(stdout, "");
    print_styled_line!(stdout, "─".repeat(60).with(colors::info()));
    print_styled!(stdout, "\n");
    stdout.flush()?;
    Ok(())
}

pub fn show_response_header() -> Result<(), std::io::Error> {
    let mut stdout = stdout();
    print_styled_line!(stdout, "─".repeat(60).with(colors::primary()));
    print_styled_line!(
        stdout,
        format!(
            "{} {}",
            "⟨⟩".with(colors::primary()).bold(),
            "Wisp's Response".bold().with(colors::text_primary())
        )
    );
    print_styled_line!(stdout, "─".repeat(60).with(colors::primary()));
    stdout.flush()?;
    Ok(())
}

pub fn create_tool_spinner(
    name: &str,
    model_name: &str,
) -> Result<ProgressBar, Box<dyn std::error::Error>> {
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::default_spinner()
            .tick_chars("˚∘○◌◯❍◉⊙✦✧⋆✨")
            .template(&format!(
                "{{spinner:.cyan}} Tool {} {} {{msg}}",
                name.with(colors::info()).bold(),
                format!("({})", model_name)
                    .with(colors::text_secondary())
                    .dim()
            ))?,
    );
    pb.set_message("running...");
    pb.enable_steady_tick(Duration::from_millis(80));
    Ok(pb)
}

pub fn show_tool_completed(
    tool_name: &str,
    model_name: &str,
    result: Option<&str>,
) -> Result<(), std::io::Error> {
    let mut stdout = stdout();
    print_styled_line!(
        stdout,
        format!(
            "{} {} {} {}",
            "✓".with(colors::success()).bold(),
            "Tool".bold().with(colors::text_primary()),
            tool_name.bold().with(colors::success()),
            format!("({})", model_name)
                .with(colors::text_secondary())
                .dim()
        )
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
                print_styled_line!(
                    stdout,
                    format!("   {} {}{}", "Result:".dim(), preview.dim(), "...".dim())
                );
            } else {
                let lines: Vec<&str> = display_result.lines().collect();
                if lines.len() == 1 {
                    print_styled_line!(
                        stdout,
                        format!("   {} {}", "Result:".dim(), &display_result.dim())
                    );
                } else {
                    print_styled_line!(stdout, format!("   {}", "Result:".dim()));
                    for line in lines.iter().take(5) {
                        print_styled_line!(stdout, format!("     {}", line.dim()));
                    }
                    if lines.len() > 5 {
                        print_styled_line!(stdout, format!("     {}", "...".dim()));
                    }
                }
            }
        }
    }
    stdout.flush()?;
    Ok(())
}

pub fn show_error(message: &str) -> Result<(), std::io::Error> {
    let mut stderr = stderr();
    print_styled_line!(
        stderr,
        format!(
            "{} {}",
            "✗".with(colors::error()).bold(),
            message.with(colors::error())
        )
    );
    stderr.flush()?;
    Ok(())
}

pub fn show_cancelled(message: &str) -> Result<(), std::io::Error> {
    let mut stderr = stderr();
    print_styled_line!(
        stderr,
        format!(
            "{} {}",
            "⊘".with(colors::warning()).bold(),
            message.with(colors::warning())
        )
    );
    stderr.flush()?;
    Ok(())
}

pub fn show_model_info(model_name: &str) -> Result<(), std::io::Error> {
    let mut stdout = stdout();
    print_styled!(
        stdout,
        format!(
            "{} ",
            format!("({})", model_name)
                .with(colors::text_secondary())
                .dim()
        )
    );
    stdout.flush()?;
    Ok(())
}

pub fn show_completion() -> Result<(), std::io::Error> {
    let mut stdout = stdout();
    print_styled_line!(stdout, "");
    print_styled_line!(stdout, "─".repeat(60).with(colors::accent()));
    print_styled_line!(
        stdout,
        format!(
            "{} {}",
            "◆".with(colors::accent()).bold(),
            "Analysis finished!".bold().with(colors::text_primary())
        )
    );
    stdout.flush()?;
    Ok(())
}
