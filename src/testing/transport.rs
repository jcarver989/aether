use rmcp::RoleClient;
use rmcp::RoleServer;
use rmcp::service::{RxJsonRpcMessage, ServiceRole, TxJsonRpcMessage};
use rmcp::transport::Transport;
use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::{Mutex, mpsc};

#[derive(Error, Debug)]
pub enum InMemoryTransportError {
    #[error("Channel closed")]
    ChannelClosed,
}

/// In-memory transport for connecting McpServer and McpClient in tests
pub struct InMemoryTransport<R: ServiceRole> {
    tx: Arc<Mutex<mpsc::UnboundedSender<TxJsonRpcMessage<R>>>>,
    rx: Arc<Mutex<mpsc::UnboundedReceiver<RxJsonRpcMessage<R>>>>,
}

impl<R: ServiceRole> InMemoryTransport<R> {
    fn new(
        tx: mpsc::UnboundedSender<TxJsonRpcMessage<R>>,
        rx: mpsc::UnboundedReceiver<RxJsonRpcMessage<R>>,
    ) -> Self {
        Self {
            tx: Arc::new(Mutex::new(tx)),
            rx: Arc::new(Mutex::new(rx)),
        }
    }
}

/// Create a pair of transports for client and server
pub fn create_transport_pair() -> (InMemoryTransport<RoleClient>, InMemoryTransport<RoleServer>) {
    // Client sends ClientRequest/ClientResult, receives ServerRequest/ServerResult
    // Server sends ServerRequest/ServerResult, receives ClientRequest/ClientResult
    let (client_tx, server_rx) = mpsc::unbounded_channel(); // Client -> Server
    let (server_tx, client_rx) = mpsc::unbounded_channel(); // Server -> Client

    let client_transport = InMemoryTransport::new(client_tx, client_rx);
    let server_transport = InMemoryTransport::new(server_tx, server_rx);

    (client_transport, server_transport)
}

impl<R: ServiceRole> Transport<R> for InMemoryTransport<R> {
    type Error = InMemoryTransportError;

    fn send(
        &mut self,
        item: TxJsonRpcMessage<R>,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send + 'static {
        let tx = self.tx.clone();
        async move {
            let tx = tx.lock().await;
            tx.send(item)
                .map_err(|_| InMemoryTransportError::ChannelClosed)?;
            Ok(())
        }
    }

    fn receive(&mut self) -> impl Future<Output = Option<RxJsonRpcMessage<R>>> + Send {
        let rx = self.rx.clone();
        async move {
            let mut rx = rx.lock().await;
            rx.recv().await
        }
    }

    async fn close(&mut self) -> Result<(), Self::Error> {
        // Channels will be closed when dropped
        Ok(())
    }
}

/// In-memory filesystem for testing
#[derive(Debug, Clone, Default)]
pub struct InMemoryFileSystem {
    files: Arc<Mutex<HashMap<String, String>>>,
}

impl InMemoryFileSystem {
    pub fn new() -> Self {
        Self {
            files: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn write_file(&self, path: &str, content: &str) -> Result<(), String> {
        let mut files = self.files.lock().await;
        files.insert(path.to_string(), content.to_string());
        Ok(())
    }

    pub async fn read_file(&self, path: &str) -> Result<String, String> {
        let files = self.files.lock().await;
        files
            .get(path)
            .cloned()
            .ok_or_else(|| format!("File not found: {}", path))
    }

    pub async fn list_files(&self) -> Result<Vec<String>, String> {
        let files = self.files.lock().await;
        Ok(files.keys().cloned().collect())
    }

    pub async fn file_exists(&self, path: &str) -> bool {
        let files = self.files.lock().await;
        files.contains_key(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_in_memory_filesystem() {
        let fs = InMemoryFileSystem::new();

        // Test writing a file
        fs.write_file("/tmp/test.txt", "Hello, World!")
            .await
            .unwrap();

        // Test reading the file
        let content = fs.read_file("/tmp/test.txt").await.unwrap();
        assert_eq!(content, "Hello, World!");

        // Test file exists
        assert!(fs.file_exists("/tmp/test.txt").await);
        assert!(!fs.file_exists("/tmp/nonexistent.txt").await);

        // Test listing files
        let files = fs.list_files().await.unwrap();
        assert_eq!(files, vec!["/tmp/test.txt"]);
    }

    #[tokio::test]
    async fn test_transport_creation() {
        let (_client, _server) = create_transport_pair();
        // Just test that we can create the transport pair without panicking
    }
}
