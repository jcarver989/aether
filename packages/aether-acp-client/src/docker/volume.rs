use super::init_container::run_init_container;
use crate::agent::DockerConfig;
use crate::error::Result;
use bollard::Docker;
use bollard::models::VolumeCreateRequest;
use bollard::models::{Mount, MountTypeEnum};
use std::collections::HashMap;
use std::path::Path;

const OVERLAY_DATA_DIR: &str = "/overlay-data";
const OVERLAY_UPPER_DIR: &str = "/overlay-data/upper";
const OVERLAY_WORK_DIR: &str = "/overlay-data/work";
const DOCKER_VOLUMES_PATH: &str = "/var/lib/docker/volumes";
const SSH_MOUNT_TARGET: &str = "/root/.ssh";

/// Tracks overlay filesystem volumes for cleanup.
#[derive(Debug)]
pub struct OverlayVolumes {
    /// Main overlay volume name (mounted as working directory)
    pub volume_name: String,
    /// Support volume name (stores upper/work directories inside the VM)
    pub writeable_volume_name: String,
}

/// Create overlay volume configuration for copy-on-write isolation.
///
/// This function:
/// 1. Creates a support volume inside the Docker VM for writeable (upper/work) directories
/// 2. Initializes the upper/work directories via a helper container
/// 3. Creates the overlay volume with proper configuration
/// 4. Returns mount configuration for the agent container
///
/// The overlay uses:
/// - lowerdir: project path on host (read-only via VirtioFS)
/// - upperdir/workdir: inside the support volume (on ext4 in the VM)
pub async fn create_overlay_volumes(
    docker: &Docker,
    container_uuid: &str,
    project_path: &Path,
    config: &DockerConfig,
) -> Result<(OverlayVolumes, Vec<Mount>)> {
    let OverlayVolumeConfig {
        overlay_volumes,
        writeable_volume_opts: support_volume_opts,
        overlay_volume_opts,
    } = OverlayVolumeConfig::new(container_uuid, project_path);

    docker.create_volume(support_volume_opts).await?;
    run_init_container(
        docker,
        "alpine:latest",
        vec![
            "mkdir".to_string(),
            "-p".to_string(),
            OVERLAY_UPPER_DIR.to_string(),
            OVERLAY_WORK_DIR.to_string(),
        ],
        vec![Mount {
            target: Some(OVERLAY_DATA_DIR.to_string()),
            source: Some(overlay_volumes.writeable_volume_name.to_string()),
            typ: Some(MountTypeEnum::VOLUME),
            ..Default::default()
        }],
    )
    .await?;
    docker.create_volume(overlay_volume_opts).await?;

    let mut mounts = vec![Mount {
        target: Some(config.working_dir.clone()),
        source: Some(overlay_volumes.volume_name.clone()),
        typ: Some(MountTypeEnum::VOLUME),
        ..Default::default()
    }];

    if let Some(ssh_dir) = dirs::home_dir()
        .map(|h| h.join(".ssh"))
        .filter(|d| d.exists())
        && config.mount_ssh_keys
    {
        mounts.push(Mount {
            target: Some(SSH_MOUNT_TARGET.to_string()),
            source: Some(ssh_dir.to_string_lossy().into_owned()),
            typ: Some(MountTypeEnum::BIND),
            read_only: Some(true),
            ..Default::default()
        });
    }

    mounts.extend(config.mounts.iter().cloned());

    Ok((overlay_volumes, mounts))
}

/// Configuration for creating overlay volumes (pure data, no side effects).
struct OverlayVolumeConfig {
    overlay_volumes: OverlayVolumes,
    writeable_volume_opts: VolumeCreateRequest,
    overlay_volume_opts: VolumeCreateRequest,
}

impl OverlayVolumeConfig {
    pub fn new(container_uuid: &str, project_path: &Path) -> Self {
        let writeable_volume_name = format!("aether-overlay-write-{container_uuid}");
        let volume_name = format!("aether-cow-{container_uuid}");
        let overlay_opts = format!(
            "lowerdir={},upperdir={}/{}/_data/upper,workdir={}/{}/_data/work",
            project_path.display(),
            DOCKER_VOLUMES_PATH,
            writeable_volume_name,
            DOCKER_VOLUMES_PATH,
            writeable_volume_name
        );

        OverlayVolumeConfig {
            overlay_volumes: OverlayVolumes {
                volume_name: volume_name.clone(),
                writeable_volume_name: writeable_volume_name.clone(),
            },

            writeable_volume_opts: VolumeCreateRequest {
                name: Some(writeable_volume_name),
                labels: Some(HashMap::from([(
                    "aether.managed".to_string(),
                    "true".to_string(),
                )])),
                ..Default::default()
            },

            overlay_volume_opts: VolumeCreateRequest {
                name: Some(volume_name.clone()),
                labels: Some(HashMap::from([(
                    "aether.managed".to_string(),
                    "true".to_string(),
                )])),
                driver: Some("local".to_string()),
                driver_opts: Some(HashMap::from([
                    ("type".to_string(), "overlay".to_string()),
                    ("device".to_string(), "overlay".to_string()),
                    ("o".to_string(), overlay_opts),
                ])),
                ..Default::default()
            },
        }
    }
}
