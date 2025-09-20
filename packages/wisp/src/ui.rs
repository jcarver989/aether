use crate::cli::ModelSpec;
use regex::Regex;
use std::io::{Write, stdout};

// Legacy CrosstermSpinner - kept for compatibility during transition
#[derive(Debug)]
pub struct CrosstermSpinner {
    message: String,
    is_running: bool,
}

impl CrosstermSpinner {
    pub fn new(tool_name: &str, model_name: &str, message: &str) -> Self {
        let formatted_message = format!(
            "Tool {} {} {}",
            tool_name,
            format!("({})", format_model_name(model_name)),
            message
        );

        Self {
            message: formatted_message,
            is_running: false,
        }
    }

    pub fn start(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.is_running = true;
        // Simple text output for now
        print!("{}", self.message);
        stdout().flush()?;
        Ok(())
    }

    pub fn finish_and_clear(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.is_running = false;
        println!(); // Just move to next line
        Ok(())
    }
}

// Simple print macros without styling
#[macro_export]
macro_rules! print_styled {
    ($writer:expr, $content:expr) => {
        write!($writer, "{}", $content)?
    };
}

#[macro_export]
macro_rules! print_styled_line {
    ($writer:expr, $content:expr) => {
        writeln!($writer, "{}", $content)?
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
    let logo_content = include_str!("./components/logo.txt");
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
                print_styled!(stdout, opacity_char.to_string());
            } else {
                print_styled!(stdout, ch.to_string());
            }
        }
        print_styled_line!(stdout, "");
    }
    print_styled_line!(stdout, "");
    let tagline_padding = " ".repeat(15);
    print_styled!(
        stdout,
        format!("{}{}", tagline_padding, "An AI agent, conjured from aether")
    );
    print_styled!(stdout, "\n\n");

    stdout.flush()?;
    Ok(())
}

pub fn show_usage(program_name: &str) -> Result<(), std::io::Error> {
    show_wisp_logo()?;
    let mut stdout = stdout();
    print_styled_line!(stdout, "Usage:");
    print_styled_line!(
        stdout,
        format!("  {} {}", program_name, "<your coding question or request>")
    );
    print_styled_line!(
        stdout,
        format!(
            "  {} {}",
            program_name, "\"help me implement a binary search tree\""
        )
    );
    stdout.flush()?;
    Ok(())
}

pub fn show_init_header(
    prompt: &str,
    model_display_name: &str,
    agents_loaded: bool,
) -> Result<(), std::io::Error> {
    let mut stdout = stdout();
    print_styled_line!(stdout, "");
    print_styled_line!(stdout, "─".repeat(60));
    print_styled_line!(stdout, format!("{} {}", "⚙", "Init"));
    print_styled_line!(stdout, "─".repeat(60));
    print_styled!(stdout, "\n");

    // Agents status
    if agents_loaded {
        print_styled!(stdout, format!("  {} System prompt: loaded AGENTS.md", "✓"));
    } else {
        print_styled!(
            stdout,
            format!("  {} No AGENTS.md file found in current directory", "ℹ")
        );
    }
    print_styled!(stdout, "\n\n");

    // User prompt
    print_styled_line!(stdout, format!("  {} User Prompt: {}", "◆", prompt));
    print_styled!(stdout, "\n");

    // Model information
    print_styled_line!(stdout, format!("  {} Model: {}", "🤖", model_display_name));
    print_styled_line!(stdout, "");
    print_styled_line!(stdout, "─".repeat(60));
    print_styled!(stdout, "\n");
    stdout.flush()?;
    Ok(())
}

pub fn create_tool_spinner(
    name: &str,
    model_name: &str,
) -> Result<CrosstermSpinner, Box<dyn std::error::Error>> {
    Ok(CrosstermSpinner::new(name, model_name, "running..."))
}

pub fn show_tool_details(
    arguments: Option<&str>,
    result: Option<&str>,
) -> Result<(), std::io::Error> {
    let mut stdout = stdout();

    // Display tool arguments/inputs
    if let Some(args) = arguments {
        if !args.trim().is_empty() {
            if args.len() > 120 {
                let truncated = &args[..117];
                print_styled_line!(stdout, format!("   Input: {}...", truncated));
            } else {
                print_styled_line!(stdout, format!("   Input: {}", args));
            }
        }
    }

    if let Some(result) = result {
        // Try to parse JSON and extract meaningful content
        let raw_result = if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(result) {
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

        // Apply escape sequence formatting
        let display_result = format_tool_result(&raw_result);

        if !display_result.trim().is_empty() {
            let lines: Vec<&str> = display_result.lines().collect();

            // For long results, show a preview with proper line handling
            if display_result.len() > 500 {
                print_styled_line!(stdout, "   Result:");
                let mut char_count = 0;
                for line in &lines {
                    if char_count + line.len() > 400 {
                        print_styled_line!(stdout, "     ...");
                        break;
                    }
                    print_styled_line!(stdout, format!("     {}", line));
                    char_count += line.len() + 1; // +1 for newline
                }
            } else if lines.len() == 1 && display_result.len() < 100 {
                // Only show as single line if it's actually short and truly single line
                print_styled_line!(stdout, format!("   Result: {}", &display_result));
            } else {
                // Always use multi-line format for better readability
                print_styled_line!(stdout, "   Result:");
                for line in lines.iter().take(10) {
                    // Show more lines since formatting is better
                    print_styled_line!(stdout, format!("     {}", line));
                }
                if lines.len() > 10 {
                    print_styled_line!(stdout, "     ...");
                }
            }
        }
    }
    stdout.flush()?;
    Ok(())
}

pub fn format_model_name(model_name: &str) -> String {
    // Parse model name and format as "provider:model"
    if let Some((provider, model)) = model_name.split_once(" (") {
        let model = model.trim_end_matches(')');
        format!("{}:{}", provider.to_lowercase(), model)
    } else {
        model_name.to_lowercase()
    }
}

pub fn format_tool_result(result: &str) -> String {
    // Handle common escape sequences to improve readability
    result
        .replace("\\n", "\n")
        .replace("\\t", "\t")
        .replace("\\r", "\r")
        .replace("\\\"", "\"")
        .replace("\\'", "'")
        .replace("\\\\", "\\")
}

pub fn format_model_display_name(model_specs: &[ModelSpec]) -> String {
    if model_specs.len() == 1 {
        let spec = &model_specs[0];
        if spec.model.is_empty() {
            format!("{:?}", spec.provider)
        } else {
            format!("{:?} ({})", spec.provider, spec.model)
        }
    } else {
        let provider_names: Vec<String> = model_specs
            .iter()
            .map(|spec| {
                if spec.model.is_empty() {
                    format!("{:?}", spec.provider)
                } else {
                    format!("{:?} ({})", spec.provider, spec.model)
                }
            })
            .collect();
        format!("Alloyed [{}]", provider_names.join(", "))
    }
}
