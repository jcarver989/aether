# aether-docker

Docker container support for Aether agents.

## Overview

`aether-docker` is a Rust crate that provides functionality to run Aether agents inside Docker containers for better isolation and reproducibility. It is designed as a separate crate to keep Docker dependencies isolated from the core aether packages.

## Features

- **Custom Dockerfiles per project**: Automatically detect and use `.aether/Dockerfile` for project-specific toolchains
- **Automatic image building and caching**: Build images once and cache them by content hash
- **Filesystem isolation**: Each agent gets an isolated view of the codebase
- **Git push capability**: Agents can push branches from inside containers to persist changes
- **SSH key mounting**: Optional mounting of SSH keys for authentication
- **Host network access**: Containers use host network for MCP servers and API access

## Requirements

- Docker daemon must be running and accessible
- User must have Docker permissions (docker group membership or rootless Docker)
- Linux, macOS, or Windows with WSL2

## Usage

Add the dependency to your `Cargo.toml`:

```toml
[dependencies]
aether-docker = { path = "../aether-docker" }
```

### Basic Example

```rust
use aether_docker::{ContainerizedAgent, ContainerConfig};
use std::path::Path;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let project_path = Path::new("/path/to/project");
    let cmd = vec!["aether-acp".to_string()];
    let config = ContainerConfig::default();

    let (agent, stdout) = ContainerizedAgent::spawn(project_path, cmd, config).await?;

    // Use the agent...
    // Read from stdout stream...

    // Cleanup
    agent.stop(10).await?;
    agent.remove(false).await?;

    Ok(())
}
```

### Custom Configuration

```rust
use aether_docker::{ContainerConfig, ContainerMount};
use std::collections::HashMap;

let mut env = HashMap::new();
env.insert("MY_VAR".to_string(), "value".to_string());

let config = ContainerConfig {
    image: Some("my-custom-image:latest".to_string()),
    mounts: vec![
        ContainerMount {
            source: "/host/path".to_string(),
            target: "/container/path".to_string(),
            read_only: true,
        }
    ],
    env,
    mount_ssh_keys: true,
    working_dir: "/workspace".to_string(),
};
```

### Custom Dockerfile

Create a `.aether/Dockerfile` in your project root:

```dockerfile
FROM ubuntu:22.04

# Install project-specific dependencies
RUN apt-get update && apt-get install -y \
    python3 \
    python3-pip \
    && rm -rf /var/lib/apt/lists/*

# Install Python packages
RUN pip3 install requests numpy

WORKDIR /workspace
```

The image will be automatically built and cached based on the Dockerfile content hash.

## Architecture

The crate provides several key components:

### DockerClient

A wrapper around the Bollard Docker client with convenience methods for:
- Creating containers with specific configurations
- Starting and stopping containers
- Executing commands in running containers
- Building images from Dockerfiles
- Pulling images from registries

### ImageBuilder

Manages Docker image building and caching:
- Detects project Dockerfiles (`.aether/Dockerfile` or `Dockerfile`)
- Builds custom images and caches by content hash
- Provides a default development image with common tools

### ContainerizedAgent

Handle to a containerized agent process:
- Spawns agents inside Docker containers
- Mounts project directory and optional SSH keys
- Provides stdin/stdout streams for ACP protocol communication
- Automatic cleanup on drop

## Testing

Run tests with:

```bash
# Unit tests (don't require Docker)
cargo test -p aether-docker --lib

# Integration tests (require Docker, some are ignored by default)
cargo test -p aether-docker --test integration_test

# Run ignored tests (requires Docker daemon)
cargo test -p aether-docker -- --ignored
```

## Error Handling

The crate uses a custom `ContainerError` enum for all Docker-related errors:

```rust
pub enum ContainerError {
    Docker(bollard::errors::Error),
    ImageBuild(String),
    AttachFailed(String),
    MountFailed(String),
    ContainerNotFound(String),
    Io(std::io::Error),
    Json(serde_json::Error),
    StartupTimeout,
    ContainerExited(i64),
}
```

## Performance Considerations

- **Container startup**: ~1-2 seconds (no worktree/branch creation needed)
- **Image building**: Can take several minutes on first build, then cached
- **Filesystem overhead**: Minimal - Docker overlay filesystem provides efficient copy-on-write

## Security Notes

- Containers run with host network (required for MCP, APIs, and git push)
- Host repository is mounted (agents can read/write within the container)
- SSH keys are optionally mounted for git push capability
- Consider the security implications of Docker socket access

## License

Same license as the parent Aether project.
