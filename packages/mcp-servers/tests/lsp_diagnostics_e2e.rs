//! End-to-end tests for LSP diagnostics through the MCP tool layer.
//!
//! These tests verify the full pipeline:
//!   file edits → LSP daemon → rust-analyzer diagnostics → queryable via `lsp_check_errors`
//!
//! Requirements:
//! - `rust-analyzer` must be installed and in PATH
//! - `aether-lspd` binary must be built (`cargo build -p aether-lspd`)
//!
//! Run with: `cargo test -p mcp-servers -- --ignored lsp_diagnostics`

use aether_lspd::testing::CargoProject;
use mcp_servers::coding::CodingMcp;
use mcp_utils::testing::connect;
use rmcp::RoleClient;
use rmcp::model::{CallToolRequestParams, ClientInfo, Implementation};
use rmcp::service::RunningService;
use std::time::Duration;

fn test_client_info() -> ClientInfo {
    ClientInfo {
        client_info: Implementation {
            name: "lsp-diagnostics-test".to_string(),
            version: "0.1.0".to_string(),
            icons: None,
            title: None,
            website_url: None,
            description: None,
        },
        ..Default::default()
    }
}

/// Call an MCP tool and parse the JSON response from the first text content block.
async fn call_tool(
    client: &RunningService<RoleClient, ClientInfo>,
    name: &str,
    args: serde_json::Value,
) -> serde_json::Value {
    let name_owned = name.to_string();
    let result = client
        .call_tool(CallToolRequestParams {
            name: name_owned.into(),
            meta: None,
            task: None,
            arguments: Some(args.as_object().unwrap().clone()),
        })
        .await
        .unwrap_or_else(|e| panic!("Failed to call tool '{name}': {e}"));

    let text = result
        .content
        .first()
        .and_then(|c| c.as_text())
        .unwrap_or_else(|| panic!("Tool '{name}' returned no text content"));

    serde_json::from_str(&text.text).unwrap_or_else(|e| {
        panic!(
            "Tool '{name}' returned invalid JSON: {e}\nRaw: {}",
            text.text
        )
    })
}

/// Poll `lsp_check_errors` until a predicate is satisfied.
async fn poll_diagnostics(
    client: &RunningService<RoleClient, ClientInfo>,
    file_path: Option<&str>,
    predicate: impl Fn(&serde_json::Value) -> bool,
) -> serde_json::Value {
    let poll_interval = Duration::from_millis(500);

    loop {
        let args = match file_path {
            Some(path) => serde_json::json!({ "file_path": path }),
            None => serde_json::json!({}),
        };

        // lsp_check_errors may fail early (e.g. LSP not ready); treat as "not yet"
        let result = call_tool(client, "lsp_check_errors", args).await;

        if predicate(&result) {
            return result;
        }

        tokio::time::sleep(poll_interval).await;
    }
}

fn has_errors(result: &serde_json::Value) -> bool {
    result
        .get("summary")
        .and_then(|s| s.get("errors"))
        .and_then(|e| e.as_u64())
        .is_some_and(|n| n > 0)
}

fn has_no_errors(result: &serde_json::Value) -> bool {
    result
        .get("summary")
        .and_then(|s| s.get("errors"))
        .and_then(|e| e.as_u64())
        .is_some_and(|n| n == 0)
}

/// Test: MCP edit_file tool → rust-analyzer picks up change → diagnostics queryable
#[tokio::test]
#[ignore]
async fn test_mcp_edit_produces_diagnostics() {
    // 1. Create a Cargo project with a type error
    let project = CargoProject::new("mcp_edit_diag").expect("Failed to create project");
    project
        .add_file(
            "src/main.rs",
            r#"fn main() {
    let x: i32 = "not an int";
    println!("{}", x);
}
"#,
        )
        .expect("Failed to add file");

    let main_rs_path = project.root().join("src/main.rs");
    let main_rs = main_rs_path.to_str().unwrap();

    // 2. Start CodingMcp with LSP enabled
    let server = CodingMcp::new().with_lsp(project.root().to_path_buf());
    let (_server_handle, client) = connect(server, test_client_info())
        .await
        .expect("Failed to connect");

    // 3. Wait for rust-analyzer to index and report the initial type error
    let result = poll_diagnostics(&client, Some(main_rs), has_errors).await;
    let errors = result["summary"]["errors"].as_u64().unwrap();
    assert!(errors > 0, "Expected type error diagnostics");

    // 4. Fix the error using MCP tools: read_file then edit_file
    call_tool(
        &client,
        "read_file",
        serde_json::json!({ "filePath": main_rs }),
    )
    .await;

    call_tool(
        &client,
        "edit_file",
        serde_json::json!({
            "filePath": main_rs,
            "oldString": "\"not an int\"",
            "newString": "42"
        }),
    )
    .await;
    // 5. Poll until errors clear
    poll_diagnostics(&client, Some(main_rs), has_no_errors).await;

    // 6. Re-introduce a different error via MCP edit
    call_tool(
        &client,
        "read_file",
        serde_json::json!({ "filePath": main_rs }),
    )
    .await;

    call_tool(
        &client,
        "edit_file",
        serde_json::json!({
            "filePath": main_rs,
            "oldString": "42",
            "newString": "true"
        }),
    )
    .await;

    // 7. Poll until errors reappear
    let result = poll_diagnostics(&client, Some(main_rs), has_errors).await;
    let errors = result["summary"]["errors"].as_u64().unwrap();
    assert!(errors > 0, "Expected type error after re-introducing bug");
}

/// Regression test: after edit_file, a SINGLE lsp_check_errors call (no polling)
/// should eventually return fresh diagnostics. This verifies the daemon waits for
/// the LSP to re-publish diagnostics after syncing a changed document.
#[tokio::test]
#[ignore]
async fn test_diagnostics_available_after_edit_without_polling() {
    // 1. Create a Cargo project with valid code
    let project =
        CargoProject::new("diag_after_edit_no_poll").expect("Failed to create project");
    project
        .add_file(
            "src/main.rs",
            r#"fn main() {
    let x: i32 = 42;
    println!("{}", x);
}
"#,
        )
        .expect("Failed to add file");

    let main_rs_path = project.root().join("src/main.rs");
    let main_rs = main_rs_path.to_str().unwrap();

    // 2. Start CodingMcp with LSP enabled
    let server = CodingMcp::new().with_lsp(project.root().to_path_buf());
    let (_server_handle, client) = connect(server, test_client_info())
        .await
        .expect("Failed to connect");

    // 3. Wait for initial indexing — no errors expected
    poll_diagnostics(&client, Some(main_rs), has_no_errors).await;

    // 4. Introduce a syntax error via edit_file
    call_tool(
        &client,
        "read_file",
        serde_json::json!({ "filePath": main_rs }),
    )
    .await;

    call_tool(
        &client,
        "edit_file",
        serde_json::json!({
            "filePath": main_rs,
            "oldString": "42",
            "newString": "\"not an int\""
        }),
    )
    .await;

    // 5. Wait a bit for rust-analyzer to process, then make a SINGLE call
    tokio::time::sleep(Duration::from_secs(3)).await;

    let result = call_tool(
        &client,
        "lsp_check_errors",
        serde_json::json!({ "file_path": main_rs }),
    )
    .await;

    let errors = result["summary"]["errors"].as_u64().unwrap_or(0);
    assert!(
        errors > 0,
        "Expected diagnostics after edit + single lsp_check_errors call, got 0 errors. \
         This indicates the daemon returns stale (empty) diagnostics after syncing a changed file. \
         Full result: {result}"
    );
}

/// Regression test: after edit_file, calling `lsp_check_errors` WITHOUT a file_path
/// (all-files mode) should still return fresh diagnostics. This verifies the daemon
/// syncs all open documents before returning the cache.
#[tokio::test]
#[ignore]
async fn test_diagnostics_all_files_after_edit() {
    // 1. Create a Cargo project with valid code
    let project = CargoProject::new("diag_all_files").expect("Failed to create project");
    project
        .add_file(
            "src/main.rs",
            r#"fn main() {
    let x: i32 = 42;
    println!("{}", x);
}
"#,
        )
        .expect("Failed to add file");

    let main_rs_path = project.root().join("src/main.rs");
    let main_rs = main_rs_path.to_str().unwrap();

    // 2. Start CodingMcp with LSP enabled
    let server = CodingMcp::new().with_lsp(project.root().to_path_buf());
    let (_server_handle, client) = connect(server, test_client_info())
        .await
        .expect("Failed to connect");

    // 3. Wait for initial indexing — no errors expected (use per-file poll to prime the cache)
    poll_diagnostics(&client, Some(main_rs), has_no_errors).await;

    // 4. Introduce a type error via edit_file
    call_tool(
        &client,
        "read_file",
        serde_json::json!({ "filePath": main_rs }),
    )
    .await;

    call_tool(
        &client,
        "edit_file",
        serde_json::json!({
            "filePath": main_rs,
            "oldString": "42",
            "newString": "\"not an int\""
        }),
    )
    .await;

    // 5. Wait a bit for rust-analyzer to process, then call lsp_check_errors WITHOUT file_path
    tokio::time::sleep(Duration::from_secs(3)).await;

    let result = call_tool(
        &client,
        "lsp_check_errors",
        serde_json::json!({}),
    )
    .await;

    let errors = result["summary"]["errors"].as_u64().unwrap_or(0);
    assert!(
        errors > 0,
        "Expected diagnostics after edit + single lsp_check_errors call (all-files mode), \
         got 0 errors. This indicates the daemon returns stale diagnostics when uri is None. \
         Full result: {result}"
    );
}

/// Regression test: after an EXTERNAL file edit (e.g. user's editor), calling
/// `lsp_check_errors {}` without file_path should detect the change and return
/// fresh diagnostics. This verifies the daemon syncs files from the diagnostics
/// cache, not just previously-opened documents.
#[tokio::test]
#[ignore]
async fn test_diagnostics_all_files_after_external_edit() {
    // 1. Create a Cargo project with valid code
    let project =
        CargoProject::new("diag_all_ext_edit").expect("Failed to create project");
    project
        .add_file(
            "src/main.rs",
            r#"fn main() {
    let x: i32 = 42;
    println!("{}", x);
}
"#,
        )
        .expect("Failed to add file");

    let main_rs_path = project.root().join("src/main.rs");
    let main_rs = main_rs_path.to_str().unwrap();

    // 2. Start CodingMcp with LSP enabled
    let server = CodingMcp::new().with_lsp(project.root().to_path_buf());
    let (_server_handle, client) = connect(server, test_client_info())
        .await
        .expect("Failed to connect");

    // 3. Wait for initial indexing — no errors expected.
    //    Use per-file poll so the diagnostics cache has an entry for this URI.
    poll_diagnostics(&client, Some(main_rs), has_no_errors).await;

    // 4. Edit the file EXTERNALLY (bypassing MCP tools), introducing a type error
    std::fs::write(
        &main_rs_path,
        r#"fn main() {
    let x: i32 = "not an int";
    println!("{}", x);
}
"#,
    )
    .expect("Failed to write file");

    // 5. Wait a bit, then call lsp_check_errors WITHOUT file_path
    tokio::time::sleep(Duration::from_secs(3)).await;

    let result = call_tool(
        &client,
        "lsp_check_errors",
        serde_json::json!({}),
    )
    .await;

    let errors = result["summary"]["errors"].as_u64().unwrap_or(0);
    assert!(
        errors > 0,
        "Expected diagnostics after external edit + lsp_check_errors (all-files mode), \
         got 0 errors. The daemon should sync files from the diagnostics cache, not just \
         open_documents. Full result: {result}"
    );
}

/// Regression test: after an EXTERNAL file edit, a SINGLE `lsp_check_errors {}`
/// call (no file_path, no polling) should return errors. The file watcher keeps the
/// diagnostics cache fresh, so the daemon should simply return whatever is cached.
#[tokio::test]
#[ignore]
async fn test_diagnostics_all_files_after_external_edit_single_call() {
    // 1. Create a Cargo project with valid code
    let project =
        CargoProject::new("diag_ext_single_call").expect("Failed to create project");
    project
        .add_file(
            "src/main.rs",
            r#"fn main() {
    let x: i32 = 42;
    println!("{}", x);
}
"#,
        )
        .expect("Failed to add file");

    let main_rs_path = project.root().join("src/main.rs");
    let main_rs = main_rs_path.to_str().unwrap();

    // 2. Start CodingMcp with LSP enabled
    let server = CodingMcp::new().with_lsp(project.root().to_path_buf());
    let (_server_handle, client) = connect(server, test_client_info())
        .await
        .expect("Failed to connect");

    // 3. Wait for initial indexing — no errors expected
    poll_diagnostics(&client, Some(main_rs), has_no_errors).await;

    // 4. Edit the file EXTERNALLY, introducing a type error
    std::fs::write(
        &main_rs_path,
        r#"fn main() {
    let x: i32 = "not an int";
    println!("{}", x);
}
"#,
    )
    .expect("Failed to write file");

    // 5. Wait for file watcher + rust-analyzer pipeline
    tokio::time::sleep(Duration::from_secs(5)).await;

    // 6. Single call — no polling. The cache should already have the errors.
    let result = call_tool(
        &client,
        "lsp_check_errors",
        serde_json::json!({}),
    )
    .await;

    let errors = result["summary"]["errors"].as_u64().unwrap_or(0);
    assert!(
        errors > 0,
        "Expected diagnostics after external edit + single lsp_check_errors call (all-files mode, no polling), \
         got 0 errors. The file watcher should have delivered fresh diagnostics to the cache. \
         Full result: {result}"
    );
}

/// Test: External fs::write → file watcher → diagnostics queryable
#[tokio::test]
#[ignore]
async fn test_external_file_change_produces_diagnostics() {
    // 1. Create a Cargo project with a type error
    let project = CargoProject::new("ext_write_diag").expect("Failed to create project");
    project
        .add_file(
            "src/main.rs",
            r#"fn main() {
    let x: i32 = "not an int";
    println!("{}", x);
}
"#,
        )
        .expect("Failed to add file");

    let main_rs_path = project.root().join("src/main.rs");
    let main_rs = main_rs_path.to_str().unwrap();

    // 2. Start CodingMcp with LSP enabled
    let server = CodingMcp::new().with_lsp(project.root().to_path_buf());
    let (_server_handle, client) = connect(server, test_client_info())
        .await
        .expect("Failed to connect");

    // 3. Wait for rust-analyzer to index and report the initial type error
    let result = poll_diagnostics(&client, Some(main_rs), has_errors).await;
    let errors = result["summary"]["errors"].as_u64().unwrap();
    assert!(errors > 0, "Expected type error diagnostics");

    // 4. Fix the error via direct filesystem write (bypassing MCP tools)
    std::fs::write(
        &main_rs_path,
        r#"fn main() {
    let x: i32 = 42;
    println!("{}", x);
}
"#,
    )
    .expect("Failed to write file");

    // 5. Poll until errors clear (file watcher → didChangeWatchedFiles → RA re-reads)
    poll_diagnostics(&client, Some(main_rs), has_no_errors).await;

    // 6. Introduce a new error via direct filesystem write
    std::fs::write(
        &main_rs_path,
        r#"fn main() {
    let x: i32 = true;
    println!("{}", x);
}
"#,
    )
    .expect("Failed to write file");

    // 7. Poll until errors reappear
    let result = poll_diagnostics(&client, Some(main_rs), has_errors).await;
    let errors = result["summary"]["errors"].as_u64().unwrap();
    assert!(errors > 0, "Expected type error after external write");
}

/// Regression test: files discovered ONLY via the file watcher (never opened or
/// present in the diagnostics cache) should still appear in all-files mode.
///
/// Unlike every other test in this file, this test does NOT prime the diagnostics
/// cache by calling `poll_diagnostics` with a file path first. Instead, it waits
/// for initial indexing to finish, then edits the file externally so the file
/// watcher fires `didChangeWatchedFiles`. If the daemon only consults
/// `diagnostics_cache.keys()` for all-files mode, this file will be invisible.
#[tokio::test]
#[ignore]
async fn test_diagnostics_all_files_discovers_file_watcher_uris() {
    // 1. Create a Cargo project with valid code
    let project =
        CargoProject::new("diag_fw_discover").expect("Failed to create project");
    project
        .add_file(
            "src/main.rs",
            r#"fn main() {
    let x: i32 = 42;
    println!("{}", x);
}
"#,
        )
        .expect("Failed to add file");

    let main_rs_path = project.root().join("src/main.rs");

    // 2. Start CodingMcp with LSP enabled
    let server = CodingMcp::new().with_lsp(project.root().to_path_buf());
    let (_server_handle, client) = connect(server, test_client_info())
        .await
        .expect("Failed to connect");

    // 3. Wait for RA to finish initial indexing WITHOUT priming the cache.
    //    We do NOT call poll_diagnostics with a file path here — that would
    //    cause ensure_document_open → didOpen → publishDiagnostics, putting
    //    main.rs into the diagnostics cache and masking the bug.
    tokio::time::sleep(Duration::from_secs(10)).await;

    // 4. Edit the file EXTERNALLY to introduce a type error.
    //    The file watcher should fire didChangeWatchedFiles.
    std::fs::write(
        &main_rs_path,
        r#"fn main() {
    let x: i32 = "not an int";
    println!("{}", x);
}
"#,
    )
    .expect("Failed to write file");

    // 5. Wait for the file watcher to detect the change and RA to re-index
    tokio::time::sleep(Duration::from_secs(5)).await;

    // 6. Call lsp_check_errors in all-files mode (no file_path).
    //    The daemon should know about main.rs via the file watcher URI set.
    let result = call_tool(
        &client,
        "lsp_check_errors",
        serde_json::json!({}),
    )
    .await;

    let errors = result["summary"]["errors"].as_u64().unwrap_or(0);
    assert!(
        errors > 0,
        "Expected diagnostics for file discovered via file watcher in all-files mode, \
         got 0 errors. The daemon should track URIs from didChangeWatchedFiles, not just \
         diagnostics_cache keys. Full result: {result}"
    );
}
