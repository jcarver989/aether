mod common;

use aether_lspd::{ClientError, LanguageId, LspClient, socket_path};
use common::{CargoProject, DaemonHarness, RA_INIT_TIMEOUT};
use std::time::Duration;

/// Test: Request on non-existent file returns appropriate response
#[tokio::test]
async fn test_nonexistent_file() {
    let project = CargoProject::new("nonexistent_test").expect("Failed to create project");

    let harness = DaemonHarness::spawn(project.root(), LanguageId::Rust)
        .await
        .expect("Failed to spawn daemon");

    let client = harness.connect().await.expect("Failed to connect");

    let uri = project.file_uri("src/main.rs");
    DaemonHarness::wait_for_lsp_ready(&client, uri, RA_INIT_TIMEOUT)
        .await
        .expect("rust-analyzer not ready");

    let fake_uri: lsp_types::Uri = "file:///nonexistent/file.rs".parse().unwrap();
    let result = client.hover(fake_uri, 0, 0).await;

    if let Ok(hover) = result {
        assert!(hover.is_none(), "Should return None for nonexistent file");
    }

    harness.kill().await.expect("Failed to kill daemon");
}

/// Test: Connection to non-running daemon fails gracefully
#[tokio::test]
async fn test_connection_to_stopped_daemon() {
    let project = CargoProject::new("stopped_daemon_test").expect("Failed to create project");

    let sock_path = socket_path(project.root(), LanguageId::Rust);

    let _ = std::fs::remove_file(&sock_path);

    let result = LspClient::connect(&sock_path, project.root(), LanguageId::Rust).await;

    assert!(
        result.is_err(),
        "Should fail to connect to non-running daemon"
    );
    match result {
        Err(ClientError::ConnectionFailed(_)) => {}
        Err(e) => panic!("Unexpected error type: {:?}", e),
        Ok(_) => panic!("Should have failed to connect"),
    }
}

/// Test: Daemon handles invalid position gracefully
#[tokio::test]
async fn test_invalid_position() {
    let project = CargoProject::new("invalid_pos_test").expect("Failed to create project");

    let content = "fn main() {}";
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

    let result = client.hover(uri.clone(), 999999, 999999).await;

    match result {
        Ok(None) => {}
        Ok(Some(_)) => {}
        Err(_) => {}
    }

    harness.kill().await.expect("Failed to kill daemon");
}

/// Test: Daemon survives client disconnection
#[tokio::test]
async fn test_client_disconnect_resilience() {
    let project = CargoProject::new("disconnect_test").expect("Failed to create project");

    let harness = DaemonHarness::spawn(project.root(), LanguageId::Rust)
        .await
        .expect("Failed to spawn daemon");

    {
        let _client = harness.connect().await.expect("Failed to connect");
    }

    tokio::time::sleep(Duration::from_millis(500)).await;

    let client2 = harness
        .connect()
        .await
        .expect("Failed to reconnect - daemon died after client disconnect");

    let uri = project.file_uri("src/main.rs");
    let _ = client2.hover(uri, 0, 0).await;

    harness.kill().await.expect("Failed to kill daemon");
}
