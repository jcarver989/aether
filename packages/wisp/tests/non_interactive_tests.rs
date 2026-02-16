use std::process::Command;

#[test]
fn test_non_interactive_help_works() {
    let output = Command::new("cargo")
        .args(["run", "-p", "wisp", "--", "--help"])
        .output()
        .expect("Failed to execute wisp --help");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("A TUI for AI coding agents via the Agent Client Protocol"),
        "Help text should be displayed"
    );
}
