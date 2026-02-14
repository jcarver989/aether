use crate::agent::{DockerProgress, ImageSource, ProgressTx};
use crate::error::{ContainerError, Result};
use bollard::Docker;
use bollard::query_parameters::{BuildImageOptions, CreateImageOptions};
use futures::StreamExt;
use sha2::{Digest, Sha256};
use std::fs::{File, read_to_string};
use std::path::Path;
use tracing::{debug, info};

/// Resolve the image source to an image tag, building or pulling as needed.
///
/// Progress updates (PullingImage, BuildingImage) are sent through the optional channel.
pub async fn resolve_image(
    docker: &Docker,
    source: &ImageSource,
    progress_tx: Option<&ProgressTx>,
) -> Result<String> {
    match source {
        ImageSource::Image(image) => {
            if docker.inspect_image(image).await.is_err() {
                if let Some(tx) = progress_tx {
                    let _ = tx.send(DockerProgress::PullingImage);
                }
                pull_image(docker, image).await?;
            }
            Ok(image.clone())
        }
        ImageSource::Dockerfile(dockerfile_path) => {
            build_dockerfile(docker, dockerfile_path, progress_tx).await
        }
    }
}

/// Pull a Docker image from a registry.
async fn pull_image(docker: &Docker, image: &str) -> Result<()> {
    info!("Pulling Docker image {image}");
    let options = CreateImageOptions {
        from_image: Some(image.to_string()),
        ..Default::default()
    };

    let mut stream = docker.create_image(Some(options), None, None);

    while let Some(msg) = stream.next().await {
        match msg {
            Ok(output) => {
                if let Some(status) = output.status {
                    debug!("Pull: {}", status);
                }

                if let Some(error_detail) = output.error_detail {
                    let msg = error_detail.message.unwrap_or_default();
                    return Err(ContainerError::ImageBuild(msg));
                }
            }
            Err(e) => return Err(ContainerError::Docker(e)),
        }
    }

    info!("Successfully pulled image {}", image);
    Ok(())
}

/// Build an image from a Dockerfile, using content-based caching.
async fn build_dockerfile(
    docker: &Docker,
    path: &Path,
    progress_tx: Option<&ProgressTx>,
) -> Result<String> {
    let dockerfile = read_to_string(path)?;
    let tag = {
        let mut hasher = Sha256::new();
        hasher.update(dockerfile.as_bytes());
        let hash = hasher.finalize();
        let hash_str = hex::encode(&hash[..8]);
        format!("aether-agent:{hash_str}")
    };

    if docker.inspect_image(&tag).await.is_ok() {
        info!("Using cached image {}", tag);
        return Ok(tag);
    }

    if let Some(tx) = progress_tx {
        let _ = tx.send(DockerProgress::BuildingImage);
    }

    info!("Building Docker image {} from {:?}", tag, path);
    let context_path = path.parent().unwrap_or(Path::new("."));
    let tar_bytes = create_tar_archive(context_path, path)?;
    let dockerfile_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("Dockerfile")
        .to_string();
    let options = BuildImageOptions {
        dockerfile: dockerfile_name,
        t: Some(tag.clone()),
        rm: true,
        ..Default::default()
    };

    let body =
        http_body_util::Either::Left(http_body_util::Full::new(bytes::Bytes::from(tar_bytes)));
    let mut stream = docker.build_image(options, None, Some(body));
    while let Some(msg) = stream.next().await {
        match msg {
            Ok(output) => {
                if let Some(stream) = output.stream {
                    debug!("Build: {}", stream.trim());
                }
                if let Some(error_detail) = output.error_detail {
                    let msg = error_detail.message.unwrap_or_default();
                    return Err(ContainerError::ImageBuild(msg));
                }
            }
            Err(e) => return Err(ContainerError::Docker(e)),
        }
    }

    info!("Successfully built image {}", tag);
    Ok(tag)
}

/// Create a tar archive of the build context.
fn create_tar_archive(context_path: &Path, dockerfile_path: &Path) -> Result<Vec<u8>> {
    let mut tar = tar::Builder::new(Vec::new());
    let mut dockerfile = File::open(dockerfile_path)?;
    let dockerfile_name = dockerfile_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("Dockerfile");

    tar.append_file(dockerfile_name, &mut dockerfile)?;
    let dockerfile_parent = dockerfile_path.parent().unwrap_or(Path::new("."));
    if context_path.exists() && context_path != dockerfile_parent {
        tar.append_dir_all(".", context_path)?;
    }

    let data = tar.into_inner()?;
    Ok(data)
}
