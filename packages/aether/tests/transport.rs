use aether::{testing::InMemoryFileSystem, transport::create_in_memory_transport};

#[tokio::test]
async fn test_in_memory_filesystem() {
    let fs = InMemoryFileSystem::new();

    // Test writing a file
    fs.write_file("/tmp/test.txt", "Hello, World!")
        .await
        .unwrap();

    // Test reading the file
    let content = fs.read_file("/tmp/test.txt").await.unwrap();
    assert_eq!(content, "Hello, World!");

    // Test file exists
    assert!(fs.file_exists("/tmp/test.txt").await);
    assert!(!fs.file_exists("/tmp/nonexistent.txt").await);

    // Test listing files
    let files = fs.list_files().await.unwrap();
    assert_eq!(files, vec!["/tmp/test.txt"]);
}

#[tokio::test]
async fn test_transport_creation() {
    let (_client, _server) = create_in_memory_transport();
    // Just test that we can create the transport pair without panicking
}
