//! Docker utilities for container spawning and management.
//!
//! This module provides low-level Docker utilities used by the agent spawner.

mod config;
mod exec;
mod image;
mod init_container;
mod volume;

pub use config::create_container_config;
pub use exec::exec_in_container;
pub use image::resolve_image;
pub use volume::{OverlayVolumes, create_overlay_volumes};
