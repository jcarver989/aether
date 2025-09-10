use rmcp::RoleClient;
use rmcp::RoleServer;
use rmcp::service::{RxJsonRpcMessage, ServiceRole, TxJsonRpcMessage};
use rmcp::transport::Transport;
use std::collections::HashMap;
use std::fmt;
use std::future::Future;
use std::sync::Arc;
use tokio::sync::{Mutex, mpsc};

#[derive(Debug)]
pub enum InMemoryTransportError {
    ChannelClosed,
}

impl fmt::Display for InMemoryTransportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InMemoryTransportError::ChannelClosed => write!(f, "Channel closed"),
        }
    }
}

impl std::error::Error for InMemoryTransportError {}

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
            .ok_or_else(|| format!("File not found: {path}"))
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
