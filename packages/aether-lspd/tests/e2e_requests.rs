mod common;

use aether_lspd::LanguageId;
use common::{CargoProject, DaemonHarness, RA_INIT_TIMEOUT, did_open_params};
use lsp_types::GotoDefinitionResponse;
use std::time::Duration;

/// Test: GotoDefinition resolves function definitions
#[tokio::test]
async fn test_goto_definition() {
    let project = CargoProject::new("goto_def_test").expect("Failed to create project");

    let content = r#"fn greet() {
    println!("Hello!");
}

fn main() {
    greet();
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

    client
        .notify_opened(did_open_params(uri.clone(), content))
        .await
        .expect("Failed to send didOpen");

    tokio::time::sleep(Duration::from_secs(3)).await;

    let result = client
        .goto_definition(uri.clone(), 5, 5)
        .await
        .expect("GotoDefinition failed");

    match result {
        GotoDefinitionResponse::Scalar(loc) => {
            assert_eq!(loc.range.start.line, 0, "Expected definition at line 0");
        }
        GotoDefinitionResponse::Array(locs) if !locs.is_empty() => {
            assert_eq!(locs[0].range.start.line, 0, "Expected definition at line 0");
        }
        GotoDefinitionResponse::Link(links) if !links.is_empty() => {
            assert_eq!(
                links[0].target_range.start.line, 0,
                "Expected definition at line 0"
            );
        }
        _ => panic!("Expected definition location, got: {:?}", result),
    }

    harness.kill().await.expect("Failed to kill daemon");
}

/// Test: Hover shows type information
#[tokio::test]
async fn test_hover() {
    let project = CargoProject::new("hover_test").expect("Failed to create project");

    let content = r#"fn main() {
    let numbers: Vec<i32> = vec![1, 2, 3];
    println!("{:?}", numbers);
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

    client
        .notify_opened(did_open_params(uri.clone(), content))
        .await
        .expect("Failed to send didOpen");

    tokio::time::sleep(Duration::from_secs(3)).await;

    let hover = client
        .hover(uri.clone(), 1, 10)
        .await
        .expect("Hover failed");

    assert!(hover.is_some(), "Expected hover information");
    let hover = hover.unwrap();

    let content_str = match &hover.contents {
        lsp_types::HoverContents::Scalar(text) => match text {
            lsp_types::MarkedString::String(s) => s.clone(),
            lsp_types::MarkedString::LanguageString(ls) => ls.value.clone(),
        },
        lsp_types::HoverContents::Array(arr) => arr
            .iter()
            .map(|m| match m {
                lsp_types::MarkedString::String(s) => s.clone(),
                lsp_types::MarkedString::LanguageString(ls) => ls.value.clone(),
            })
            .collect::<Vec<_>>()
            .join("\n"),
        lsp_types::HoverContents::Markup(markup) => markup.value.clone(),
    };

    assert!(
        content_str.contains("Vec") || content_str.contains("i32"),
        "Hover should mention Vec or i32, got: {}",
        content_str
    );

    harness.kill().await.expect("Failed to kill daemon");
}

/// Test: FindReferences locates all usages
#[tokio::test]
async fn test_find_references() {
    let project = CargoProject::new("refs_test").expect("Failed to create project");

    let content = r#"fn helper() -> i32 {
    42
}

fn main() {
    let a = helper();
    let b = helper();
    println!("{} {}", a, b);
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

    client
        .notify_opened(did_open_params(uri.clone(), content))
        .await
        .expect("Failed to send didOpen");

    tokio::time::sleep(Duration::from_secs(3)).await;

    let refs = client
        .find_references(uri.clone(), 0, 4, true)
        .await
        .expect("FindReferences failed");

    assert!(
        refs.len() >= 2,
        "Expected at least 2 references to helper(), got {}",
        refs.len()
    );

    harness.kill().await.expect("Failed to kill daemon");
}

/// Test: DocumentSymbol returns symbols
#[tokio::test]
async fn test_document_symbol() {
    let project = CargoProject::new("symbol_test").expect("Failed to create project");

    let content = r#"struct Point {
    x: i32,
    y: i32,
}

fn distance(p1: &Point, p2: &Point) -> f64 {
    0.0
}

fn main() {
    let p = Point { x: 0, y: 0 };
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

    client
        .notify_opened(did_open_params(uri.clone(), content))
        .await
        .expect("Failed to send didOpen");

    tokio::time::sleep(Duration::from_secs(3)).await;

    let symbols = client
        .document_symbol(uri.clone())
        .await
        .expect("DocumentSymbol failed");

    let symbol_names: Vec<String> = match symbols {
        lsp_types::DocumentSymbolResponse::Flat(syms) => {
            syms.iter().map(|s| s.name.clone()).collect()
        }
        lsp_types::DocumentSymbolResponse::Nested(syms) => {
            syms.iter().map(|s| s.name.clone()).collect()
        }
    };

    assert!(
        symbol_names.iter().any(|n| n == "Point"),
        "Should find Point struct, got: {:?}",
        symbol_names
    );
    assert!(
        symbol_names.iter().any(|n| n == "distance"),
        "Should find distance function, got: {:?}",
        symbol_names
    );
    assert!(
        symbol_names.iter().any(|n| n == "main"),
        "Should find main function, got: {:?}",
        symbol_names
    );

    harness.kill().await.expect("Failed to kill daemon");
}

/// Test: WorkspaceSymbol searches across files
#[tokio::test]
async fn test_workspace_symbol() {
    let project = CargoProject::new("ws_symbol_test").expect("Failed to create project");

    project
        .add_file(
            "src/main.rs",
            r#"mod utils;
fn main() { utils::special_helper(); }
"#,
        )
        .expect("Failed to add main.rs");

    project
        .add_file(
            "src/utils.rs",
            r#"pub fn special_helper() {}
"#,
        )
        .expect("Failed to add utils.rs");

    let harness = DaemonHarness::spawn(project.root(), LanguageId::Rust)
        .await
        .expect("Failed to spawn daemon");

    let client = harness.connect().await.expect("Failed to connect");

    let uri = project.file_uri("src/main.rs");
    DaemonHarness::wait_for_lsp_ready(&client, uri.clone(), RA_INIT_TIMEOUT)
        .await
        .expect("rust-analyzer not ready");

    tokio::time::sleep(Duration::from_secs(5)).await;

    let symbols = client
        .workspace_symbol("special_helper".to_string())
        .await
        .expect("WorkspaceSymbol failed");

    assert!(!symbols.is_empty(), "Should find special_helper symbol");
    assert!(
        symbols.iter().any(|s| s.name == "special_helper"),
        "Should find special_helper in results: {:?}",
        symbols
    );

    harness.kill().await.expect("Failed to kill daemon");
}
