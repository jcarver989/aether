use crate::transport::InMemoryFileSystem;

/// A simple mock tool that writes to the in-memory filesystem
/// This simulates what a real MCP server tool would do
pub async fn mock_write_file_tool(
    filesystem: &InMemoryFileSystem,
    path: &str,
    content: &str,
) -> Result<String, String> {
    filesystem.write_file(path, content).await?;
    Ok(format!(
        "Successfully wrote {} bytes to {}",
        content.len(),
        path
    ))
}
