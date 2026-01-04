//! Platform abstraction layer.
//!
//! This module centralizes all platform-specific (`#[cfg(feature = "desktop")]`)
//! re-exports so consumer modules can import from a single location.

pub use crate::events::AgentEvent;

#[cfg(feature = "desktop")]
pub use crate::acp_agent::AgentHandle;
#[cfg(not(feature = "desktop"))]
pub use crate::fakes::acp_agent::AgentHandle;

pub use crate::file_types::FileMatch;

#[cfg(not(feature = "desktop"))]
pub use crate::fakes::file_search::{FileSearcher, FileSearcherCache};
#[cfg(feature = "desktop")]
pub use crate::file_search::{FileSearcher, FileSearcherCache};

#[cfg(not(feature = "desktop"))]
pub use crate::fakes::DockerProgress;
#[cfg(feature = "desktop")]
pub use aether_acp_client::DockerProgress;

#[cfg(not(feature = "desktop"))]
pub use futures::channel::mpsc;
#[cfg(feature = "desktop")]
pub use tokio::sync::mpsc;

#[cfg(not(feature = "desktop"))]
pub use futures::channel::oneshot;
#[cfg(feature = "desktop")]
pub use tokio::sync::oneshot;

#[cfg(not(feature = "desktop"))]
pub use futures::lock::Mutex;
#[cfg(feature = "desktop")]
pub use tokio::sync::Mutex;

pub mod io;
pub mod resources;
pub mod voice;
pub use voice::RecordingState;

// =============================================================================
// Channel helpers
// =============================================================================

/// Create an unbounded channel for sending events.
///
/// Desktop: Uses `tokio::sync::mpsc::unbounded_channel()`
/// Web: Uses `futures::channel::mpsc::unbounded()`
pub fn unbounded_channel<T>() -> (mpsc::UnboundedSender<T>, mpsc::UnboundedReceiver<T>) {
    #[cfg(feature = "desktop")]
    {
        mpsc::unbounded_channel()
    }
    #[cfg(not(feature = "desktop"))]
    {
        mpsc::unbounded()
    }
}

// =============================================================================
// Receiver extension trait
// =============================================================================

/// Extension trait for unified receiver polling across platforms.
///
/// Desktop (tokio): Uses `.recv().await`
/// Web (futures): Uses `.next().await` via `StreamExt`
#[cfg(feature = "desktop")]
pub trait ReceiverExt<T> {
    /// Receive the next item, returning `None` when the channel is closed.
    fn recv_next(&mut self) -> impl std::future::Future<Output = Option<T>> + Send;
}

#[cfg(feature = "desktop")]
impl<T: Send> ReceiverExt<T> for mpsc::UnboundedReceiver<T> {
    async fn recv_next(&mut self) -> Option<T> {
        self.recv().await
    }
}

#[cfg(not(feature = "desktop"))]
pub trait ReceiverExt<T> {
    /// Receive the next item, returning `None` when the channel is closed.
    fn recv_next(&mut self) -> impl std::future::Future<Output = Option<T>>;
}

#[cfg(not(feature = "desktop"))]
impl<T> ReceiverExt<T> for mpsc::UnboundedReceiver<T> {
    async fn recv_next(&mut self) -> Option<T> {
        use futures::StreamExt;
        self.next().await
    }
}
