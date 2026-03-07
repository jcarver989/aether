mod common;

use aether_lspd::{ClientError, LanguageId};
use common::{CargoProject, DaemonHarness, RA_INIT_TIMEOUT, TestProject, did_open_params};
use lsp_types::GotoDefinitionResponse;
use std::future::Future;
use std::time::Duration;

/// Retry an LSP request while RA is still indexing.
///
/// Retries on any `LspError` (e.g. "content modified", "file not found") and
/// also when `should_retry` returns true for a successful result (e.g. empty
/// results or `{unknown}` types before RA finishes inference).
async fn retry_lsp<T, F, Fut>(
    timeout: Duration,
    mut f: F,
    should_retry: impl Fn(&T) -> bool,
) -> Result<T, ClientError>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, ClientError>>,
{
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        let at_deadline = tokio::time::Instant::now() >= deadline;
        match f().await {
            Err(ClientError::LspError { .. }) if !at_deadline => {
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
            Ok(val) if should_retry(&val) && !at_deadline => {
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
            result => return result,
        }
    }
}

fn is_goto_empty(r: &GotoDefinitionResponse) -> bool {
    match r {
        GotoDefinitionResponse::Scalar(_) => false,
        GotoDefinitionResponse::Array(a) => a.is_empty(),
        GotoDefinitionResponse::Link(l) => l.is_empty(),
    }
}

fn extract_hover_text(hover: Option<&lsp_types::Hover>) -> Option<String> {
    let hover = hover?;
    let text = match &hover.contents {
        lsp_types::HoverContents::Scalar(t) => match t {
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
    Some(text)
}

/// Test: `GotoDefinition` resolves function definitions
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

    let result = retry_lsp(
        RA_INIT_TIMEOUT,
        || client.goto_definition(uri.clone(), 5, 5),
        is_goto_empty,
    )
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
        _ => panic!("Expected definition location, got: {result:?}"),
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

    // RA may return None or {unknown} types while still inferring — retry until resolved
    let hover = retry_lsp(
        RA_INIT_TIMEOUT,
        || client.hover(uri.clone(), 1, 10),
        |h| extract_hover_text(h.as_ref()).is_none_or(|t| !t.contains("Vec") && !t.contains("i32")),
    )
    .await
    .expect("Hover failed");

    let content_str = extract_hover_text(hover.as_ref()).expect("Expected hover information");

    assert!(
        content_str.contains("Vec") || content_str.contains("i32"),
        "Hover should mention Vec or i32, got: {content_str}"
    );

    harness.kill().await.expect("Failed to kill daemon");
}

/// Test: `FindReferences` locates all usages
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

    let refs = retry_lsp(
        RA_INIT_TIMEOUT,
        || client.find_references(uri.clone(), 0, 4, true),
        |r| r.len() < 2,
    )
    .await
    .expect("FindReferences failed");

    assert!(
        refs.len() >= 2,
        "Expected at least 2 references to helper(), got {}",
        refs.len()
    );

    harness.kill().await.expect("Failed to kill daemon");
}

/// Test: `DocumentSymbol` returns symbols
#[tokio::test]
async fn test_document_symbol() {
    let project = CargoProject::new("symbol_test").expect("Failed to create project");

    let content = r"struct Point {
    x: i32,
    y: i32,
}

fn distance(p1: &Point, p2: &Point) -> f64 {
    0.0
}

fn main() {
    let p = Point { x: 0, y: 0 };
}
";
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

    let symbols = retry_lsp(
        RA_INIT_TIMEOUT,
        || client.document_symbol(uri.clone()),
        |s| match s {
            lsp_types::DocumentSymbolResponse::Flat(v) => v.is_empty(),
            lsp_types::DocumentSymbolResponse::Nested(v) => v.is_empty(),
        },
    )
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
        "Should find Point struct, got: {symbol_names:?}"
    );
    assert!(
        symbol_names.iter().any(|n| n == "distance"),
        "Should find distance function, got: {symbol_names:?}"
    );
    assert!(
        symbol_names.iter().any(|n| n == "main"),
        "Should find main function, got: {symbol_names:?}"
    );

    harness.kill().await.expect("Failed to kill daemon");
}

/// Test: `WorkspaceSymbol` searches across files
#[tokio::test]
async fn test_workspace_symbol() {
    let project = CargoProject::new("ws_symbol_test").expect("Failed to create project");

    project
        .add_file(
            "src/main.rs",
            r"mod utils;
fn main() { utils::special_helper(); }
",
        )
        .expect("Failed to add main.rs");

    project
        .add_file(
            "src/utils.rs",
            r"pub fn special_helper() {}
",
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

    let symbols = retry_lsp(
        RA_INIT_TIMEOUT,
        || client.workspace_symbol("special_helper".to_string()),
        std::vec::Vec::is_empty,
    )
    .await
    .expect("WorkspaceSymbol failed");

    assert!(!symbols.is_empty(), "Should find special_helper symbol");
    assert!(
        symbols.iter().any(|s| s.name == "special_helper"),
        "Should find special_helper in results: {symbols:?}"
    );

    harness.kill().await.expect("Failed to kill daemon");
}
