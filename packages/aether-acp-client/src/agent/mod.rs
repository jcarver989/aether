//! Agent spawning abstractions for local and containerized processes.
//!
//! This module provides a unified interface for spawning ACP agents regardless of
//! whether they run as local processes or inside Docker containers.

mod agent_spawner;
mod docker_spawner;
mod local_spawner;

pub use agent_spawner::{
    AgentError, AgentInput, AgentOutput, AgentProcess, DockerConfig, DockerProgress, ImageSource,
    ProgressRx, ProgressTx, SpawnConfig, spawn_agent_process,
};
pub use docker_spawner::DockerAgentProcess;
pub use local_spawner::LocalAgentProcess;
