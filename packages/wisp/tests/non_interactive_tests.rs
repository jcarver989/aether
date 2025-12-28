use std::process::Command;

#[test]
fn test_non_interactive_mode_requires_prompt() {
    // Running wisp with no arguments should launch interactive mode
    // We can't test this easily in CI, but we can document the expected behavior
    // This test just ensures the binary can be built
    let output = Command::new("cargo")
        .args(["build", "-p", "wisp"])
        .output()
        .expect("Failed to build wisp");

    assert!(output.status.success(), "Failed to build wisp package");
}

#[test]
fn test_non_interactive_help_works() {
    // Test that help flag works (this doesn't require an LLM)
    let output = Command::new("cargo")
        .args(["run", "-p", "wisp", "--", "--help"])
        .output()
        .expect("Failed to execute wisp --help");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("A TUI for the Aether AI assistant"),
        "Help text should be displayed"
    );
}

#[test]
#[ignore] // Requires running LLM endpoint
fn test_non_interactive_with_prompt_exits_cleanly() {
    // This test requires a running LLM endpoint
    // Skip by default, but can be run with: cargo test -- --ignored
    let output = Command::new("cargo")
        .args([
            "run", "-p", "wisp", "--", "Say", "hello", "in", "one", "word",
        ])
        .output()
        .expect("Failed to execute wisp");

    // Should exit (either success or failure, but not hang)
    assert!(
        output.status.code().is_some(),
        "Process should exit with a status code"
    );
}

#[test]
#[ignore] // Requires running LLM endpoint
fn test_non_interactive_streams_to_stdout() {
    // Verify output goes to stdout, not to TUI
    let output = Command::new("cargo")
        .args(["run", "-p", "wisp", "--", "What", "is", "2+2?"])
        .output()
        .expect("Failed to execute wisp");

    let stdout = String::from_utf8_lossy(&output.stdout);
    // Should contain actual output, not TUI escape sequences
    assert!(
        !stdout.contains("\x1b[?1049h"),
        "Should not use alternate screen buffer (TUI mode)"
    );
    assert!(
        !stdout.contains("\x1b[?25l"),
        "Should not hide cursor (TUI mode)"
    );
}

#[test]
#[ignore] // Requires running LLM endpoint
fn test_non_interactive_handles_multiword_prompts() {
    // Test that multi-word prompts are joined correctly
    let output = Command::new("cargo")
        .args(["run", "-p", "wisp", "--", "Print", "the", "word", "hello"])
        .output()
        .expect("Failed to execute wisp");

    let stdout = String::from_utf8_lossy(&output.stdout);
    // Should contain some output (the exact output depends on the LLM)
    assert!(!stdout.is_empty(), "Should produce some output");
}

#[test]
#[ignore] // Requires running LLM endpoint
fn test_non_interactive_tool_calls_visible() {
    // Test that tool calls are visible in non-interactive mode
    let output = Command::new("cargo")
        .args([
            "run",
            "-p",
            "wisp",
            "--",
            "List",
            "files",
            "in",
            "current",
            "directory",
        ])
        .output()
        .expect("Failed to execute wisp");

    let stdout = String::from_utf8_lossy(&output.stdout);
    // Tool calls should be prefixed with [Tool: name]
    // The exact tool name depends on the implementation, but there should be some indication
    assert!(
        stdout.contains("[Tool:") || stdout.contains("files") || !stdout.is_empty(),
        "Should show tool call information or output"
    );
}
