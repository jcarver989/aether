use crate::testing::{InMemoryFileSystem, create_transport_pair};
use rmcp::transport::Transport;

/// Simple integration test that verifies the transport works
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_transport_pair_creation() {
        // Test that we can create transport pairs without panicking
        let (_client_transport, _server_transport) = create_transport_pair();
        // This test passes if no panic occurs during creation
    }

    #[tokio::test]
    async fn test_in_memory_filesystem_integration() {
        let filesystem = InMemoryFileSystem::new();

        // Test writing a file
        filesystem
            .write_file("/test/integration.txt", "Integration test content")
            .await
            .expect("Failed to write file");

        // Test reading the file
        let content = filesystem
            .read_file("/test/integration.txt")
            .await
            .expect("Failed to read file");
        assert_eq!(content, "Integration test content");

        // Test file exists
        assert!(filesystem.file_exists("/test/integration.txt").await);
        assert!(!filesystem.file_exists("/test/nonexistent.txt").await);

        // Test listing files
        let files = filesystem.list_files().await.expect("Failed to list files");
        assert_eq!(files, vec!["/test/integration.txt"]);

        // Test writing multiple files
        filesystem
            .write_file("/test/file1.txt", "Content 1")
            .await
            .expect("Failed to write file1");
        filesystem
            .write_file("/test/file2.txt", "Content 2")
            .await
            .expect("Failed to write file2");

        let files = filesystem.list_files().await.expect("Failed to list files");
        assert_eq!(files.len(), 3);
        assert!(files.contains(&"/test/integration.txt".to_string()));
        assert!(files.contains(&"/test/file1.txt".to_string()));
        assert!(files.contains(&"/test/file2.txt".to_string()));
    }

    #[tokio::test]
    async fn test_transport_closes_properly() {
        let (mut client_transport, mut server_transport) = create_transport_pair();

        // Test that close works without error
        client_transport
            .close()
            .await
            .expect("Failed to close client transport");
        server_transport
            .close()
            .await
            .expect("Failed to close server transport");
    }
}

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

#[cfg(test)]
mod tool_tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_tool_integration() {
        let filesystem = InMemoryFileSystem::new();

        // Simulate calling a tool that writes to the filesystem
        let result = mock_write_file_tool(&filesystem, "/tools/test.txt", "Tool content")
            .await
            .expect("Tool should succeed");

        assert_eq!(result, "Successfully wrote 12 bytes to /tools/test.txt");

        // Verify the file was actually written
        let content = filesystem
            .read_file("/tools/test.txt")
            .await
            .expect("File should exist");
        assert_eq!(content, "Tool content");
    }
}
