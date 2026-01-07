//! ACP Client library for Aether.
//!
//! This crate provides:
//!
//! - **Agent spawning**: Unified interface for spawning ACP agents locally or in Docker
//! - **ACP Client**: Implementation of the ACP `Client` trait for handling agent requests
//! - **Session management**: Functions for establishing ACP sessions
//! - **Event transformation**: Utilities for transforming protocol events
//!
//! # Agent Spawning Example
//!
//! ```no_run
//! use aether_acp_client::{spawn_agent_process, SpawnConfig, DockerConfig, ImageSource};
//! use std::path::Path;
//! use std::collections::HashMap;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let project_path = Path::new("/path/to/project");
//!     let cmd = vec!["aether-acp".to_string()];
//!
//!     // Option 1: Spawn as local process
//!     let (agent, input, output) = spawn_agent_process(
//!         SpawnConfig::Local,
//!         project_path,
//!         cmd.clone(),
//!         None, // No progress channel for local spawn
//!     ).await?;
//!
//!     // Option 2: Spawn in Docker container with progress tracking
//!     let docker_config = DockerConfig {
//!         image: ImageSource::Image("ubuntu:22.04".to_string()),
//!         mounts: vec![],
//!         env: HashMap::new(),
//!         mount_ssh_keys: true,
//!         working_dir: "/workspace".to_string(),
//!     };
//!     let (agent, input, output) = spawn_agent_process(
//!         SpawnConfig::Docker(docker_config),
//!         project_path,
//!         cmd,
//!         None, // Pass Some(tx) to receive progress updates
//!     ).await?;
//!
//!     // Returns Arc<dyn AgentProcess> for lifecycle operations
//!     agent.exec(vec!["ls".to_string()]).await?;
//!
//!     Ok(())
//! }
//! ```

// Agent spawning
mod agent;
mod docker;
pub mod error;

// ACP client and protocol helpers
pub mod client;
pub mod session;
pub mod transform;

// Re-exports: Agent spawning
pub use agent::{
    AgentError, AgentInput, AgentOutput, AgentProcess, DockerAgentProcess, DockerConfig,
    DockerProgress, ImageSource, LocalAgentProcess, ProgressRx, ProgressTx, SpawnConfig,
    spawn_agent_process,
};
pub use bollard::models::Mount;
pub use error::{ContainerError, Result};

// Re-exports: ACP client
pub use client::{AcpClient, OutputStream, RawAgentEvent};
pub use session::{SessionError, SessionInfo, start_session};
pub use transform::{
    AcpEvent, extract_tool_content, transform_raw_event, transform_session_notification,
};
