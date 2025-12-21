//! JSON-RPC transport layer for LSP communication over stdio
//!
//! LSP uses a simple framing protocol:
//! - Headers: `Content-Length: <number>\r\n\r\n`
//! - Body: JSON-RPC message

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::{ChildStdin, ChildStdout};

use super::error::{LspError, Result};

/// A JSON-RPC request message
#[derive(Debug, Serialize)]
pub struct JsonRpcRequest<P: Serialize> {
    pub jsonrpc: &'static str,
    pub id: i64,
    pub method: String,
    pub params: P,
}

impl<P: Serialize> JsonRpcRequest<P> {
    pub fn new(id: i64, method: impl Into<String>, params: P) -> Self {
        Self {
            jsonrpc: "2.0",
            id,
            method: method.into(),
            params,
        }
    }
}

/// A JSON-RPC notification message (no id, no response expected)
#[derive(Debug, Serialize)]
pub struct JsonRpcNotification<P: Serialize> {
    pub jsonrpc: &'static str,
    pub method: String,
    pub params: P,
}

impl<P: Serialize> JsonRpcNotification<P> {
    pub fn new(method: impl Into<String>, params: P) -> Self {
        Self {
            jsonrpc: "2.0",
            method: method.into(),
            params,
        }
    }
}

/// A JSON-RPC response message
#[derive(Debug, Deserialize)]
pub struct JsonRpcResponse {
    #[allow(dead_code)]
    pub jsonrpc: String,
    pub id: Option<i64>,
    pub result: Option<Value>,
    pub error: Option<JsonRpcError>,
}

/// A JSON-RPC error object
#[derive(Debug, Deserialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[allow(dead_code)]
    pub data: Option<Value>,
}

/// An incoming message from the LSP server (could be response or notification)
#[derive(Debug, Deserialize)]
pub struct IncomingMessage {
    #[allow(dead_code)]
    pub jsonrpc: String,
    pub id: Option<i64>,
    pub method: Option<String>,
    pub params: Option<Value>,
    pub result: Option<Value>,
    pub error: Option<JsonRpcError>,
}

impl IncomingMessage {
    /// Returns true if this is a response to a request (has id and result/error)
    pub fn is_response(&self) -> bool {
        self.id.is_some() && (self.result.is_some() || self.error.is_some())
    }

    /// Returns true if this is a notification (has method but no id)
    pub fn is_notification(&self) -> bool {
        self.method.is_some() && self.id.is_none()
    }
}

/// Read a single LSP message from the server's stdout
///
/// This parses the Content-Length header and reads the JSON body.
pub async fn read_message(reader: &mut BufReader<ChildStdout>) -> Result<IncomingMessage> {
    let mut content_length: Option<usize> = None;

    // Read headers until we find Content-Length
    loop {
        let mut header = String::new();
        let bytes_read = reader.read_line(&mut header).await?;

        if bytes_read == 0 {
            return Err(LspError::Transport("Server closed connection".into()));
        }

        let header = header.trim();

        // Empty line signals end of headers
        if header.is_empty() {
            break;
        }

        // Parse Content-Length header
        if let Some(value) = header.strip_prefix("Content-Length: ") {
            content_length = Some(value.parse().map_err(|_| {
                LspError::InvalidMessage(format!("Invalid Content-Length: {}", value))
            })?);
        }
    }

    let content_length = content_length
        .ok_or_else(|| LspError::InvalidMessage("Missing Content-Length header".into()))?;

    // Read the JSON body
    let mut content = vec![0u8; content_length];
    reader.read_exact(&mut content).await?;

    let message: IncomingMessage = serde_json::from_slice(&content)?;
    Ok(message)
}

/// Write a JSON-RPC request to the server's stdin
pub async fn write_request<P: Serialize>(
    writer: &mut ChildStdin,
    request: &JsonRpcRequest<P>,
) -> Result<()> {
    let content = serde_json::to_string(request)?;
    write_raw_message(writer, &content).await
}

/// Write a JSON-RPC notification to the server's stdin
pub async fn write_notification<P: Serialize>(
    writer: &mut ChildStdin,
    notification: &JsonRpcNotification<P>,
) -> Result<()> {
    let content = serde_json::to_string(notification)?;
    write_raw_message(writer, &content).await
}

/// Write a raw message with LSP framing
async fn write_raw_message(writer: &mut ChildStdin, content: &str) -> Result<()> {
    let header = format!("Content-Length: {}\r\n\r\n", content.len());
    writer.write_all(header.as_bytes()).await?;
    writer.write_all(content.as_bytes()).await?;
    writer.flush().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_json_rpc_request_serialization() {
        let request = JsonRpcRequest::new(1, "initialize", serde_json::json!({"processId": 123}));
        let json = serde_json::to_string(&request).unwrap();

        assert!(json.contains("\"jsonrpc\":\"2.0\""));
        assert!(json.contains("\"id\":1"));
        assert!(json.contains("\"method\":\"initialize\""));
        assert!(json.contains("\"processId\":123"));
    }

    #[test]
    fn test_json_rpc_notification_serialization() {
        let notification = JsonRpcNotification::new("initialized", serde_json::json!({}));
        let json = serde_json::to_string(&notification).unwrap();

        assert!(json.contains("\"jsonrpc\":\"2.0\""));
        assert!(json.contains("\"method\":\"initialized\""));
        assert!(!json.contains("\"id\"")); // Notifications don't have id
    }

    #[test]
    fn test_incoming_message_is_response() {
        let response: IncomingMessage =
            serde_json::from_str(r#"{"jsonrpc":"2.0","id":1,"result":{"capabilities":{}}}"#)
                .unwrap();

        assert!(response.is_response());
        assert!(!response.is_notification());
        assert_eq!(response.id, Some(1));
    }

    #[test]
    fn test_incoming_message_is_notification() {
        let notification: IncomingMessage = serde_json::from_str(
            r#"{"jsonrpc":"2.0","method":"textDocument/publishDiagnostics","params":{}}"#,
        )
        .unwrap();

        assert!(!notification.is_response());
        assert!(notification.is_notification());
        assert_eq!(
            notification.method.as_deref(),
            Some("textDocument/publishDiagnostics")
        );
    }
}
