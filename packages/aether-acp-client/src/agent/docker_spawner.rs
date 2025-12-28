use super::agent_spawner::{
    AgentError, AgentInput, AgentOutput, AgentProcess, DockerConfig, DockerProgress, ProgressTx,
};
use crate::docker::{
    OverlayVolumes, create_container_config, create_overlay_volumes, exec_in_container,
    resolve_image,
};
use crate::error::ContainerError;
use async_trait::async_trait;
use bollard::Docker;
use bollard::container::{
    AttachContainerOptions, RemoveContainerOptions, StartContainerOptions, StopContainerOptions,
};
use bollard::exec::StartExecResults;
use bytes::Bytes;
use futures::StreamExt;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};
use tokio_util::io::StreamReader;
use tracing::{debug, warn};

/// A Docker container agent handle for lifecycle management.
///
/// This struct is `Sync` and implements `Agent` for use with `Arc<dyn Agent>`.
/// IO streams are returned separately from `spawn()` since they are not `Sync`.
pub struct DockerAgentProcess {
    container_id: String,
    container_name: String,
    docker: Arc<Docker>,
    project_path: PathBuf,
    working_dir: String,
    overlay: OverlayVolumes,
}

impl DockerAgentProcess {
    /// Spawn a new agent in a Docker container.
    ///
    /// Returns the agent handle and IO streams separately. The handle implements
    /// `Agent` and can be used with `Arc<dyn Agent>`. The IO streams are for
    /// passing to `ClientSideConnection::new()`.
    ///
    /// Progress updates are sent through the optional `progress_tx` channel.
    pub async fn spawn(
        config: &DockerConfig,
        project_path: &Path,
        cmd: Vec<String>,
        progress_tx: Option<ProgressTx>,
    ) -> Result<(Self, AgentInput, AgentOutput), AgentError> {
        let docker = Docker::connect_with_local_defaults()?;

        if let Some(ref tx) = progress_tx {
            let _ = tx.send(DockerProgress::CheckingImage);
        }

        let image = resolve_image(&docker, &config.image, progress_tx.as_ref()).await?;

        if let Some(ref tx) = progress_tx {
            let _ = tx.send(DockerProgress::CreatingVolumes);
        }

        let container_uuid = uuid::Uuid::new_v4();
        let container_name = format!("aether-agent-{container_uuid}");
        let (overlay_dirs, mounts) =
            create_overlay_volumes(&docker, &container_uuid.to_string(), project_path, config)
                .await?;

        if let Some(ref tx) = progress_tx {
            let _ = tx.send(DockerProgress::StartingContainer);
        }

        let (container_id, attach_result) = {
            let env: Vec<String> = config.env.iter().map(|(k, v)| format!("{k}={v}")).collect();

            let (create_options, container_config) = create_container_config(
                &container_name,
                &image,
                mounts,
                env,
                cmd,
                Some(config.working_dir.clone()),
            );

            let response = docker
                .create_container(Some(create_options), container_config)
                .await?;

            let container_id = response.id;
            docker
                .start_container(&container_id, None::<StartContainerOptions<String>>)
                .await?;

            let attach_result = docker
                .attach_container(
                    &container_id,
                    Some(AttachContainerOptions::<String> {
                        stdin: Some(true),
                        stdout: Some(true),
                        stderr: Some(true),
                        stream: Some(true),
                        ..Default::default()
                    }),
                )
                .await?;

            (container_id, attach_result)
        };

        let input: AgentInput = Box::pin(attach_result.input.compat_write());
        let output: AgentOutput = {
            let stdout_stream = attach_result.output.map(|result| {
                result
                    .map(|log_output| Bytes::from(log_output.into_bytes().to_vec()))
                    .map_err(|e| io::Error::other(e.to_string()))
            });

            let stdout_reader = StreamReader::new(stdout_stream);
            Box::pin(stdout_reader.compat())
        };

        let agent = Self {
            container_id,
            container_name,
            docker: Arc::new(docker),
            project_path: project_path.to_path_buf(),
            working_dir: config.working_dir.clone(),
            overlay: overlay_dirs,
        };

        Ok((agent, input, output))
    }

    /// Get the container name.
    pub fn container_name(&self) -> &str {
        &self.container_name
    }
}

#[async_trait]
impl AgentProcess for DockerAgentProcess {
    async fn terminate(&self, timeout_secs: i64) -> Result<(), AgentError> {
        debug!("Stopping container {}", self.container_id);
        let stop_options = StopContainerOptions { t: timeout_secs };
        self.docker
            .stop_container(&self.container_id, Some(stop_options))
            .await
            .map_err(ContainerError::Docker)?;

        debug!("Removing container {}", self.container_id);
        let remove_options = RemoveContainerOptions {
            force: true,
            v: true,
            ..Default::default()
        };
        self.docker
            .remove_container(&self.container_id, Some(remove_options))
            .await
            .map_err(ContainerError::Docker)?;

        debug!("Removing overlay volume {}", self.overlay.volume_name);
        if let Err(e) = self
            .docker
            .remove_volume(&self.overlay.volume_name, None)
            .await
        {
            warn!("Failed to remove overlay volume: {}", e);
        }

        debug!(
            "Removing support volume {}",
            self.overlay.writeable_volume_name
        );
        if let Err(e) = self
            .docker
            .remove_volume(&self.overlay.writeable_volume_name, None)
            .await
        {
            warn!("Failed to remove support volume: {}", e);
        }

        Ok(())
    }

    fn id(&self) -> &str {
        &self.container_id
    }

    async fn exec(&self, cmd: Vec<String>) -> Result<String, AgentError> {
        if cmd.is_empty() {
            return Err(AgentError::Spawn("Empty command".to_string()));
        }

        let output = exec_in_container(
            &self.docker,
            &self.container_id,
            cmd,
            Some(self.working_dir.clone()),
        )
        .await?;

        let mut result = String::new();
        if let StartExecResults::Attached { mut output, .. } = output {
            while let Some(msg) = output.next().await {
                match msg {
                    Ok(log_output) => {
                        result.push_str(&String::from_utf8_lossy(&log_output.into_bytes()));
                    }
                    Err(e) => {
                        return Err(AgentError::Spawn(format!("Exec output error: {}", e)));
                    }
                }
            }
        }

        Ok(result)
    }

    fn project_path(&self) -> &Path {
        &self.project_path
    }
}

#[cfg(test)]
mod tests {
    use super::super::agent_spawner::ImageSource;
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_docker_config_with_image() {
        let config = DockerConfig {
            image: ImageSource::Image("alpine:latest".to_string()),
            mounts: vec![],
            env: HashMap::new(),
            mount_ssh_keys: true,
            working_dir: "/workspace".to_string(),
        };
        assert_eq!(config.working_dir, "/workspace");
        assert!(matches!(config.image, ImageSource::Image(ref s) if s == "alpine:latest"));
    }

    #[test]
    fn test_docker_config_with_dockerfile() {
        let config = DockerConfig {
            image: ImageSource::Dockerfile(PathBuf::from("/path/to/Dockerfile")),
            mounts: vec![],
            env: HashMap::new(),
            mount_ssh_keys: true,
            working_dir: "/workspace".to_string(),
        };
        assert!(matches!(config.image, ImageSource::Dockerfile(_)));
    }
}
