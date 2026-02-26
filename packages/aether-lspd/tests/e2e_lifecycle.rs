mod common;

use aether_lspd::LanguageId;
use common::{CargoProject, DaemonHarness, RA_INIT_TIMEOUT, TestProject, did_change_params, did_open_params};
use lsp_types::{DidCloseTextDocumentParams, TextDocumentIdentifier};
use std::time::Duration;

/// Test: Full lifecycle - initialize, didOpen, diagnostics, disconnect
#[tokio::test]
async fn test_full_lifecycle() {
    let project = CargoProject::new("lifecycle_test").expect("Failed to create project");

    let content = r#"
fn main() {
    let x: i32 = "not an int";
}
"#;
    project
        .add_file("src/main.rs", content)
        .expect("Failed to add file");

    let harness = DaemonHarness::spawn(project.root(), LanguageId::Rust)
        .await
        .expect("Failed to spawn daemon");

    let client = harness.connect().await.expect("Failed to connect");

    let uri = project.file_uri("src/main.rs");
    DaemonHarness::wait_for_lsp_ready(&client, uri.clone(), RA_INIT_TIMEOUT)
        .await
        .expect("rust-analyzer not ready");

    let open_params = did_open_params(uri.clone(), content);
    client
        .notify_opened(open_params)
        .await
        .expect("Failed to send didOpen");

    tokio::time::sleep(Duration::from_secs(5)).await;

    let diagnostics = client
        .get_diagnostics(Some(uri.clone()))
        .await
        .expect("Failed to get diagnostics");

    let has_errors = diagnostics.iter().any(|d| !d.diagnostics.is_empty());
    assert!(has_errors, "Expected diagnostics for type error");

    harness.kill().await.expect("Failed to kill daemon");
}

/// Test: Multiple clients sharing same daemon
#[tokio::test]
async fn test_multiple_clients() {
    let project = CargoProject::new("multi_client_test").expect("Failed to create project");

    let content = std::fs::read_to_string(project.root().join("src/main.rs"))
        .expect("Failed to read main.rs");

    let harness = DaemonHarness::spawn(project.root(), LanguageId::Rust)
        .await
        .expect("Failed to spawn daemon");

    let client1 = harness.connect().await.expect("Failed to connect client 1");

    let client2 = harness.connect().await.expect("Failed to connect client 2");

    let uri = project.file_uri("src/main.rs");
    DaemonHarness::wait_for_lsp_ready(&client1, uri.clone(), RA_INIT_TIMEOUT)
        .await
        .expect("rust-analyzer not ready");

    client1
        .notify_opened(did_open_params(uri.clone(), &content))
        .await
        .expect("Failed to send didOpen");

    tokio::time::sleep(Duration::from_secs(2)).await;

    let hover1 = client1.hover(uri.clone(), 0, 0).await;
    let hover2 = client2.hover(uri.clone(), 0, 0).await;

    assert!(hover1.is_ok(), "Client 1 hover failed: {:?}", hover1);
    assert!(hover2.is_ok(), "Client 2 hover failed: {:?}", hover2);

    harness.kill().await.expect("Failed to kill daemon");
}

/// Test: Document changes trigger re-analysis (didChange notification works)
#[tokio::test]
async fn test_did_change_notification() {
    let project = CargoProject::new("change_test").expect("Failed to create project");

    let invalid_content = r#"fn main() { let x: i32 = "not an int"; }"#;
    project
        .add_file("src/main.rs", invalid_content)
        .expect("Failed to add file");

    let harness = DaemonHarness::spawn(project.root(), LanguageId::Rust)
        .await
        .expect("Failed to spawn daemon");

    let client = harness.connect().await.expect("Failed to connect");

    let uri = project.file_uri("src/main.rs");
    DaemonHarness::wait_for_lsp_ready(&client, uri.clone(), RA_INIT_TIMEOUT)
        .await
        .expect("rust-analyzer not ready");

    client
        .notify_opened(did_open_params(uri.clone(), invalid_content))
        .await
        .expect("Failed to send didOpen");

    tokio::time::sleep(Duration::from_secs(5)).await;

    let diag1 = client
        .get_diagnostics(Some(uri.clone()))
        .await
        .expect("Failed to get diagnostics");
    let has_errors = diag1
        .iter()
        .flat_map(|d| d.diagnostics.iter())
        .any(|d| d.severity == Some(lsp_types::DiagnosticSeverity::ERROR));

    assert!(has_errors, "Expected errors in invalid code");

    let valid_content = r#"fn main() { let x: i32 = 42; }"#;
    client
        .notify_changed(did_change_params(uri.clone(), 2, valid_content))
        .await
        .expect("Failed to send didChange");

    tokio::time::sleep(Duration::from_secs(2)).await;

    let hover = client.hover(uri.clone(), 0, 3).await;
    assert!(
        hover.is_ok(),
        "Hover should work after didChange: {:?}",
        hover
    );

    harness.kill().await.expect("Failed to kill daemon");
}

/// Test: Diagnostics are shared across clients
#[tokio::test]
async fn test_diagnostics_shared_across_clients() {
    let project = CargoProject::new("shared_diag_test").expect("Failed to create project");

    let content = r#"
fn main() {
    let x: i32 = "not an int";
}
"#;
    project
        .add_file("src/main.rs", content)
        .expect("Failed to add file");

    let harness = DaemonHarness::spawn(project.root(), LanguageId::Rust)
        .await
        .expect("Failed to spawn daemon");

    let client1 = harness.connect().await.expect("Failed to connect client 1");

    let uri = project.file_uri("src/main.rs");
    DaemonHarness::wait_for_lsp_ready(&client1, uri.clone(), RA_INIT_TIMEOUT)
        .await
        .expect("rust-analyzer not ready");

    // Client 1 opens the file
    client1
        .notify_opened(did_open_params(uri.clone(), content))
        .await
        .expect("Failed to send didOpen");

    tokio::time::sleep(Duration::from_secs(5)).await;

    // Verify client 1 sees diagnostics
    let diag1 = client1
        .get_diagnostics(Some(uri.clone()))
        .await
        .expect("Failed to get diagnostics");
    assert!(
        diag1.iter().any(|d| !d.diagnostics.is_empty()),
        "Client 1 should see diagnostics"
    );

    // Client 2 connects and should also see diagnostics (cached)
    let client2 = harness.connect().await.expect("Failed to connect client 2");
    let diag2 = client2
        .get_diagnostics(Some(uri.clone()))
        .await
        .expect("Failed to get diagnostics");
    assert!(
        diag2.iter().any(|d| !d.diagnostics.is_empty()),
        "Client 2 should see cached diagnostics"
    );

    harness.kill().await.expect("Failed to kill daemon");
}

/// Test: Get all diagnostics (None uri)
#[tokio::test]
async fn test_get_all_diagnostics() {
    let project = CargoProject::new("all_diag_test").expect("Failed to create project");

    let main_content = r#"
mod lib;
fn main() {
    let x: i32 = "error1";
}
"#;
    let lib_content = r#"
pub fn foo() {
    let y: i32 = "error2";
}
"#;
    project
        .add_file("src/main.rs", main_content)
        .expect("Failed to add main.rs");
    project
        .add_file("src/lib.rs", lib_content)
        .expect("Failed to add lib.rs");

    let harness = DaemonHarness::spawn(project.root(), LanguageId::Rust)
        .await
        .expect("Failed to spawn daemon");

    let client = harness.connect().await.expect("Failed to connect");

    let main_uri = project.file_uri("src/main.rs");
    let lib_uri = project.file_uri("src/lib.rs");

    DaemonHarness::wait_for_lsp_ready(&client, main_uri.clone(), RA_INIT_TIMEOUT)
        .await
        .expect("rust-analyzer not ready");

    // Open both files
    client
        .notify_opened(did_open_params(main_uri.clone(), main_content))
        .await
        .expect("Failed to open main.rs");
    client
        .notify_opened(did_open_params(lib_uri.clone(), lib_content))
        .await
        .expect("Failed to open lib.rs");

    tokio::time::sleep(Duration::from_secs(5)).await;

    // Get all diagnostics (None uri)
    let all_diags = client
        .get_diagnostics(None)
        .await
        .expect("Failed to get all diagnostics");

    // Should have diagnostics from at least one file
    let total_errors: usize = all_diags
        .iter()
        .flat_map(|d| d.diagnostics.iter())
        .filter(|d| d.severity == Some(lsp_types::DiagnosticSeverity::ERROR))
        .count();

    assert!(total_errors >= 1, "Expected at least 1 error across files");

    harness.kill().await.expect("Failed to kill daemon");
}

/// Test: didClose notification
#[tokio::test]
async fn test_did_close_notification() {
    let project = CargoProject::new("close_test").expect("Failed to create project");

    let content = r#"fn main() { println!("Hello"); }"#;
    project
        .add_file("src/main.rs", content)
        .expect("Failed to add file");

    let harness = DaemonHarness::spawn(project.root(), LanguageId::Rust)
        .await
        .expect("Failed to spawn daemon");

    let client = harness.connect().await.expect("Failed to connect");

    let uri = project.file_uri("src/main.rs");
    DaemonHarness::wait_for_lsp_ready(&client, uri.clone(), RA_INIT_TIMEOUT)
        .await
        .expect("rust-analyzer not ready");

    // Open the file
    client
        .notify_opened(did_open_params(uri.clone(), content))
        .await
        .expect("Failed to send didOpen");

    tokio::time::sleep(Duration::from_secs(2)).await;

    // Close the file
    let close_params = DidCloseTextDocumentParams {
        text_document: TextDocumentIdentifier { uri: uri.clone() },
    };
    client
        .notify_closed(close_params)
        .await
        .expect("Failed to send didClose");

    // Verify daemon still works after close
    let hover = client.hover(uri.clone(), 0, 0).await;
    // Hover may return None for closed file, but shouldn't error
    assert!(
        hover.is_ok(),
        "Hover should not error after didClose: {:?}",
        hover
    );

    harness.kill().await.expect("Failed to kill daemon");
}
