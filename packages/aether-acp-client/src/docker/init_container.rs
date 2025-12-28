//! One-shot container execution for initialization tasks.
//!
//! Provides a generic way to run ephemeral containers that execute a single
//! command and clean up after themselves.

use crate::error::{ContainerError, Result};
use bollard::Docker;
use bollard::container::{
    Config, CreateContainerOptions, RemoveContainerOptions, StartContainerOptions,
    WaitContainerOptions,
};
use bollard::image::CreateImageOptions;
use bollard::models::{HostConfig, Mount};
use futures::StreamExt;
use futures::TryStreamExt;
use tracing::{debug, info};

/// Run a one-shot container with a specific image.
pub async fn run_init_container(
    docker: &Docker,
    image: &str,
    cmd: Vec<String>,
    mounts: Vec<Mount>,
) -> Result<()> {
    let container_name = format!("aether-init-{}", uuid::Uuid::new_v4());
    create_image_if_not_exists(docker, image).await?;

    docker
        .create_container(
            Some(CreateContainerOptions {
                name: container_name.clone(),
                ..Default::default()
            }),
            Config {
                image: Some(image.to_string()),
                cmd: Some(cmd),
                host_config: Some(HostConfig {
                    mounts: Some(mounts),
                    ..Default::default()
                }),
                ..Default::default()
            },
        )
        .await?;

    docker
        .start_container(&container_name, None::<StartContainerOptions<String>>)
        .await?;

    let mut wait_stream = docker.wait_container(
        &container_name,
        Some(WaitContainerOptions {
            condition: "not-running",
        }),
    );

    while let Some(result) = wait_stream.try_next().await? {
        if result.status_code != 0 {
            remove_container(docker, &container_name).await;
            return Err(ContainerError::ContainerExited(result.status_code));
        }
    }

    remove_container(docker, &container_name).await;
    debug!("Init container completed successfully");
    Ok(())
}

async fn create_image_if_not_exists(docker: &Docker, image: &str) -> Result<()> {
    if docker.inspect_image(image).await.is_ok() {
        return Ok(());
    }

    info!("Pulling {image} for init container");
    let mut stream = docker.create_image(
        Some(CreateImageOptions {
            from_image: image,
            ..Default::default()
        }),
        None,
        None,
    );

    while let Some(result) = stream.next().await {
        result?;
    }

    Ok(())
}

async fn remove_container(docker: &Docker, name: &str) {
    let _ = docker
        .remove_container(
            name,
            Some(RemoveContainerOptions {
                force: true,
                v: true,
                ..Default::default()
            }),
        )
        .await;
}
