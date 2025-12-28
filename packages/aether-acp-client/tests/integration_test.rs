use aether_acp_client::{AgentProcess, DockerAgentProcess, DockerConfig, ImageSource};
use futures::io::AsyncReadExt;
use std::collections::HashMap;
use tempfile::TempDir;

/// Integration test for spawning a container (requires Docker).
/// This test is ignored by default as it requires Docker to be running.
#[tokio::test]
#[ignore]
async fn test_spawn_container() {
    let temp_dir = TempDir::new().unwrap();

    let config = DockerConfig {
        image: ImageSource::Image("alpine:latest".to_string()),
        mounts: vec![],
        env: HashMap::new(),
        mount_ssh_keys: false,
        working_dir: "/workspace".to_string(),
    };

    let cmd = vec!["sh".to_string(), "-c".to_string(), "echo Hello".to_string()];

    let result = DockerAgentProcess::spawn(&config, temp_dir.path(), cmd, None).await;

    match result {
        Ok((agent, _input, mut output)) => {
            // Read some output using AsyncRead
            let mut buffer = vec![0u8; 1024];
            match output.read(&mut buffer).await {
                Ok(n) if n > 0 => {
                    let output_str = String::from_utf8_lossy(&buffer[..n]);
                    assert!(output_str.contains("Hello") || output_str.is_empty());
                }
                _ => {}
            }

            // Cleanup
            let _ = agent.terminate(5).await;
        }
        Err(e) => {
            panic!("Failed to spawn container: {}", e);
        }
    }
}
