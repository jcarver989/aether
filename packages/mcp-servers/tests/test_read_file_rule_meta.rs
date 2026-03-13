use mcp_servers::coding::CodingMcp;
use mcp_servers::coding::tools::read_file::ReadFileArgs;
use std::fs;
use tempfile::TempDir;

#[tokio::test]
async fn test_read_file_meta_includes_matched_rule_names() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    // Set up a skill with a read trigger for .rs files
    let skill_dir = root.join(".aether").join("skills").join("writing-rust");
    fs::create_dir_all(&skill_dir).unwrap();
    fs::write(
        skill_dir.join("SKILL.md"),
        "---\ndescription: Rust conventions\ntriggers:\n  read:\n    - \"**/*.rs\"\n---\nRust best practices.\n",
    )
    .unwrap();

    // Create an .rs file to read
    let src_dir = root.join("src");
    fs::create_dir_all(&src_dir).unwrap();
    let rs_file = src_dir.join("main.rs");
    fs::write(&rs_file, "fn main() {}\n").unwrap();

    let mcp = CodingMcp::new().with_root_dir(root.to_path_buf());

    let result = mcp
        .test_read_file(ReadFileArgs {
            file_path: rs_file.to_string_lossy().to_string(),
            offset: None,
            limit: None,
        })
        .await
        .unwrap();

    let meta = result
        .0
        .meta
        .as_ref()
        .expect("_meta should be set when rules match");
    assert_eq!(meta.display.title, "Read file");
    assert!(
        meta.display.value.contains("+rules: writing-rust"),
        "expected '+rules: writing-rust' in display value, got: {}",
        meta.display.value
    );
    assert!(
        meta.display.value.contains("1 lines"),
        "expected line count in display value, got: {}",
        meta.display.value
    );
}

#[tokio::test]
async fn test_read_file_meta_unchanged_without_rules() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    // No skills set up — just a plain file
    let txt_file = root.join("readme.txt");
    fs::write(&txt_file, "hello\nworld\n").unwrap();

    let mcp = CodingMcp::new();

    let result = mcp
        .test_read_file(ReadFileArgs {
            file_path: txt_file.to_string_lossy().to_string(),
            offset: None,
            limit: None,
        })
        .await
        .unwrap();

    let meta = result.0.meta.as_ref().expect("_meta should always be set");
    // Should NOT contain "+rules:" since no rules matched
    assert!(
        !meta.display.value.contains("+rules:"),
        "expected no '+rules:' in display value, got: {}",
        meta.display.value
    );
}
