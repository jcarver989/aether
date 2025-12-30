//! Container configuration builder.

use bollard::container::{Config, CreateContainerOptions};
use bollard::models::{HostConfig, Mount};

/// Create container configuration with the specified parameters.
pub fn create_container_config(
    name: &str,
    image: &str,
    mounts: Vec<Mount>,
    env: Vec<String>,
    cmd: Vec<String>,
    working_dir: Option<String>,
) -> (CreateContainerOptions<String>, Config<String>) {
    let host_config = HostConfig {
        mounts: Some(mounts),
        network_mode: Some("host".to_string()),
        ..Default::default()
    };

    let config = Config {
        image: Some(image.to_string()),
        env: Some(env),
        cmd: Some(cmd),
        working_dir,
        host_config: Some(host_config),
        attach_stdin: Some(true),
        attach_stdout: Some(true),
        attach_stderr: Some(true),
        open_stdin: Some(true),
        stdin_once: Some(true),
        tty: Some(false),
        ..Default::default()
    };

    let options = CreateContainerOptions {
        name: name.to_string(),
        ..Default::default()
    };

    (options, config)
}
