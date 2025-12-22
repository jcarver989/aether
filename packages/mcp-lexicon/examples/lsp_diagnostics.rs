//! Example: Connect to rust-analyzer and get diagnostics
//!
//! This example demonstrates how to:
//! 1. Spawn rust-analyzer for a Rust project
//! 2. Open a file and wait for diagnostics
//! 3. Modify the file to introduce an error
//! 4. Observe the new diagnostics
//!
//! Usage:
//!   cargo run -p mcp_lexicon --example lsp_diagnostics -- /path/to/rust/project
//!
//! Requirements:
//! - rust-analyzer must be installed and in PATH
//! - The target project must be a valid Rust project with Cargo.toml

use std::collections::HashSet;
use std::env;
use std::path::PathBuf;
use std::time::Duration;

use lsp_types::{
    DidChangeTextDocumentParams, DidOpenTextDocumentParams, NumberOrString, ProgressParamsValue,
    TextDocumentContentChangeEvent, TextDocumentItem, VersionedTextDocumentIdentifier,
    WorkDoneProgress,
};
use mcp_lexicon::coding::lsp::{
    ClientNotification, LspClient, NotificationReceiver, ServerNotification, count_by_severity,
    format_diagnostics, path_to_uri,
};
use tokio::time::timeout;

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

    println!("Starting rust-analyzer for: {}", project_path.display());

    let (tx, mut rx, mut client) = match LspClient::spawn("rust-analyzer", &[], &project_path).await
    {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to spawn rust-analyzer: {}", e);
            std::process::exit(1);
        }
    };

    println!("Language server initialized.");
    println!("Waiting for workspace indexing to complete...");

    wait_for_indexing(&mut rx).await;
    println!("Indexing complete!");

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

    println!("Opening file: {}", target_file.display());

    let original_content = std::fs::read_to_string(&target_file)?;
    let file_uri = path_to_uri(&target_file)?;

    // Open the file in the language server via the tx channel
    tx.send(ClientNotification::TextDocumentOpened(
        DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: file_uri.clone(),
                language_id: "rust".to_string(),
                version: 1,
                text: original_content.clone(),
            },
        },
    ))
    .await?;

    // Wait for initial diagnostics
    println!("\nWaiting for initial diagnostics...");
    let initial_diagnostics =
        wait_for_diagnostics(&mut rx, &file_uri, Duration::from_secs(60)).await;

    print_diagnostics_summary(initial_diagnostics.as_ref());

    // Introduce a syntax error by appending garbage
    println!("\n--- Introducing a syntax error ---");
    let broken_content = format!("{}\n\nthis_is_not_valid_rust_code!!!", original_content);
    tx.send(ClientNotification::TextDocumentChanged(
        DidChangeTextDocumentParams {
            text_document: VersionedTextDocumentIdentifier {
                uri: file_uri.clone(),
                version: 2,
            },
            content_changes: vec![TextDocumentContentChangeEvent {
                range: None,
                range_length: None,
                text: broken_content,
            }],
        },
    ))
    .await?;

    // Wait for new diagnostics
    println!("Waiting for error diagnostics...");
    let error_diagnostics = wait_for_diagnostics(&mut rx, &file_uri, Duration::from_secs(60)).await;

    print_diagnostics_summary(error_diagnostics.as_ref());

    // Fix the file by restoring original content
    println!("\n--- Restoring original content ---");
    tx.send(ClientNotification::TextDocumentChanged(
        DidChangeTextDocumentParams {
            text_document: VersionedTextDocumentIdentifier {
                uri: file_uri.clone(),
                version: 3,
            },
            content_changes: vec![TextDocumentContentChangeEvent {
                range: None,
                range_length: None,
                text: original_content,
            }],
        },
    ))
    .await?;

    // Wait for diagnostics to clear
    println!("Waiting for diagnostics after fix...");
    let fixed_diagnostics = wait_for_diagnostics(&mut rx, &file_uri, Duration::from_secs(60)).await;

    print_diagnostics_summary(fixed_diagnostics.as_ref());

    // Shutdown
    println!("\nShutting down...");
    client.shutdown().await?;
    println!("Done!");

    Ok(())
}

/// Wait for the server to finish indexing/loading the workspace
///
/// This waits for all in-progress work to complete by tracking $/progress notifications.
async fn wait_for_indexing(rx: &mut NotificationReceiver) {
    let mut active_tokens: HashSet<String> = HashSet::new();

    loop {
        match timeout(Duration::from_millis(500), rx.recv()).await {
            Ok(Some(ServerNotification::Progress(progress))) => {
                let token = match progress.token {
                    NumberOrString::Number(n) => n.to_string(),
                    NumberOrString::String(s) => s,
                };
                let ProgressParamsValue::WorkDone(work_done) = progress.value;

                match work_done {
                    WorkDoneProgress::Begin(_) => {
                        active_tokens.insert(token);
                    }
                    WorkDoneProgress::End(_) => {
                        active_tokens.remove(&token);
                    }
                    WorkDoneProgress::Report(_) => {}
                }
            }
            Ok(Some(_)) => {
                // Ignore non-progress notifications during indexing wait
            }
            Ok(None) => break, // Channel closed
            Err(_) => {
                // Timeout - if no active tokens, we're done
                if active_tokens.is_empty() {
                    break;
                }
            }
        }
    }
}

/// Wait for diagnostics for a specific file URI
///
/// Blocks until we receive diagnostics for the target file, with a safety timeout.
async fn wait_for_diagnostics(
    rx: &mut NotificationReceiver,
    target_uri: &lsp_types::Uri,
    safety_timeout: Duration,
) -> Option<lsp_types::PublishDiagnosticsParams> {
    // Wait for diagnostics for our specific file
    match timeout(safety_timeout, async {
        loop {
            match rx.recv().await {
                Some(ServerNotification::Diagnostics(diag))
                    if diag.uri.as_str() == target_uri.as_str() =>
                {
                    return Some(diag);
                }
                Some(_) => continue, // Other notification or different file, keep waiting
                None => return None, // Channel closed
            }
        }
    })
    .await
    {
        Ok(result) => result,
        Err(_) => {
            eprintln!("Timeout waiting for diagnostics");
            None
        }
    }
}

/// Print a summary of diagnostics
fn print_diagnostics_summary(diagnostics: Option<&lsp_types::PublishDiagnosticsParams>) {
    match diagnostics {
        Some(params) => {
            let formatted = format_diagnostics(params);
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
        None => {
            println!("\nNo diagnostics received (channel closed or timeout)");
        }
    }
}
