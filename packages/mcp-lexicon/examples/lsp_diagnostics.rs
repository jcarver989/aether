//! Example: Get diagnostics using LspCodingTools
//!
//! This example demonstrates how to:
//! 1. Wrap DefaultCodingTools with LspCodingTools for multi-language LSP integration
//! 2. Read/write files (which automatically spawns the appropriate LSP and notifies it)
//! 3. Query diagnostics through the tools abstraction
//!
//! Usage:
//!   cargo run -p mcp_lexicon --example lsp_diagnostics -- /path/to/rust/project
//!
//! Requirements:
//! - rust-analyzer must be installed and in PATH (for Rust projects)
//! - The target project must be a valid Rust project with Cargo.toml

use std::env;
use std::path::PathBuf;
use std::time::Duration;

use lsp_types::{Diagnostic, Uri};
use mcp_lexicon::coding::lsp::{count_by_severity, path_to_uri, FormattedDiagnostic};
use mcp_lexicon::coding::tools::read_file::ReadFileArgs;
use mcp_lexicon::coding::tools::write_file::WriteFileArgs;
use mcp_lexicon::coding::{CodingTools, DefaultCodingTools, LspCodingTools};
use tokio::time::sleep;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let project_path = env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| env::current_dir().expect("Failed to get current directory"));

    if !project_path.join("Cargo.toml").exists() {
        eprintln!(
            "Error: {} is not a Rust project (no Cargo.toml found)",
            project_path.display()
        );
        std::process::exit(1);
    }

    println!("Creating LspCodingTools for: {}", project_path.display());

    // LspCodingTools automatically detects and spawns the appropriate LSP
    // based on file extension (e.g., rust-analyzer for .rs files)
    let tools = LspCodingTools::new(DefaultCodingTools::new(), project_path.clone());

    println!("LspCodingTools created (LSP will be spawned lazily on first file access).");

    // Find target file
    let lib_rs = project_path.join("src/lib.rs");
    let main_rs = project_path.join("src/main.rs");

    let target_file = if lib_rs.exists() {
        lib_rs
    } else if main_rs.exists() {
        main_rs
    } else {
        eprintln!("No src/lib.rs or src/main.rs found in the project");
        std::process::exit(1);
    };

    let target_file_str = target_file.to_string_lossy().to_string();
    let file_uri = path_to_uri(&target_file)?;

    println!("Opening file: {}", target_file.display());

    // Read the file - this automatically spawns rust-analyzer and notifies it
    let read_result = tools
        .read_file(ReadFileArgs {
            file_path: target_file_str.clone(),
            offset: None,
            limit: None,
        })
        .await?;

    let original_content = read_result.raw_content.clone();
    println!("Read {} bytes from file", original_content.len());

    // Wait for initial diagnostics
    println!("\nWaiting for initial diagnostics...");
    let initial_diagnostics = wait_for_diagnostics(&tools, &file_uri).await;
    print_diagnostics_summary(&initial_diagnostics, &file_uri);

    // Introduce a syntax error by appending garbage
    println!("\n--- Introducing a syntax error ---");
    let broken_content = format!("{}\n\nthis_is_not_valid_rust_code!!!", original_content);

    // Write the broken content - this automatically notifies the LSP of the change
    tools
        .write_file(WriteFileArgs {
            file_path: target_file_str.clone(),
            content: broken_content,
        })
        .await?;

    // Wait for error diagnostics
    println!("Waiting for error diagnostics...");
    let error_diagnostics = wait_for_diagnostics(&tools, &file_uri).await;
    print_diagnostics_summary(&error_diagnostics, &file_uri);

    // Fix the file by restoring original content
    println!("\n--- Restoring original content ---");
    tools
        .write_file(WriteFileArgs {
            file_path: target_file_str,
            content: original_content,
        })
        .await?;

    // Wait for diagnostics to clear
    println!("Waiting for diagnostics after fix...");
    let fixed_diagnostics = wait_for_diagnostics(&tools, &file_uri).await;
    print_diagnostics_summary(&fixed_diagnostics, &file_uri);

    println!("\nDone!");

    Ok(())
}

/// Wait for diagnostics by polling until the cache has an entry for our file
async fn wait_for_diagnostics<T: CodingTools>(tools: &T, target_uri: &Uri) -> Vec<Diagnostic> {
    let target_uri_str = target_uri.to_string();
    loop {
        sleep(Duration::from_millis(500)).await;

        if let Ok(cache) = tools.get_lsp_diagnostics().await {
            if let Some(diagnostics) = cache.get(&target_uri_str) {
                return diagnostics.clone();
            }
        }
    }
}

/// Print a summary of diagnostics
fn print_diagnostics_summary(diagnostics: &[Diagnostic], uri: &Uri) {
    let formatted: Vec<FormattedDiagnostic> = diagnostics
        .iter()
        .map(|d| FormattedDiagnostic::from_diagnostic(uri, d))
        .collect();

    let counts = count_by_severity(&formatted);

    println!("\nDiagnostics summary: {}", counts);

    if formatted.is_empty() {
        println!("  (no diagnostics)");
    } else {
        for diag in &formatted {
            println!("  {}", diag.format());
        }
    }
}
