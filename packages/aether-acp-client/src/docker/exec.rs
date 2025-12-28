//! Docker exec command execution.

use crate::error::{ContainerError, Result};
use bollard::Docker;
use bollard::exec::{CreateExecOptions, StartExecResults};
use tracing::debug;

/// Execute a command in a running container.
///
/// Returns the `StartExecResults` which can be used to read stdout/stderr.
pub async fn exec_in_container(
    docker: &Docker,
    container_id: &str,
    cmd: Vec<String>,
    working_dir: Option<String>,
) -> Result<StartExecResults> {
    debug!("Executing command in container {}: {:?}", container_id, cmd);

    let exec_opts = CreateExecOptions {
        cmd: Some(cmd),
        attach_stdout: Some(true),
        attach_stderr: Some(true),
        working_dir,
        ..Default::default()
    };

    let exec = docker
        .create_exec(container_id, exec_opts)
        .await
        .map_err(ContainerError::Docker)?;

    docker
        .start_exec(&exec.id, None)
        .await
        .map_err(ContainerError::Docker)
}
