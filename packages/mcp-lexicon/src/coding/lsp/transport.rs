//! JSON-RPC transport layer for LSP communication over stdio
//!
//! LSP uses a simple framing protocol:
//! - Headers: `Content-Length: <number>\r\n\r\n`
//! - Body: JSON-RPC message

use lsp_types::{
    DidChangeTextDocumentParams, DidOpenTextDocumentParams, DidSaveTextDocumentParams,
    ProgressParams, PublishDiagnosticsParams,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, to_value};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::{ChildStdin, ChildStdout};

use super::error::{LspError, Result};

/// A parsed incoming message from the LSP server
#[derive(Debug)]
pub enum ParsedMessage {
    /// Response to a request we sent
    Response(ResponseMessage),
    /// Server-initiated notification
    Notification(ParsedNotification),
}

/// A response to a request we sent to the server
#[derive(Debug)]
pub struct ResponseMessage {
    pub id: i64,
    pub result: Option<Value>,
    pub error: Option<JsonRpcError>,
}

/// Server notifications we support, with typed payloads
#[derive(Debug)]
pub enum ParsedNotification {
    /// Diagnostics published for a document (errors, warnings, etc.)
    Diagnostics(PublishDiagnosticsParams),
    /// Progress updates (indexing, loading workspace, etc.)
    Progress(ProgressParams),
    /// Unknown notification (method name preserved for logging)
    Unknown(String),
}

/// Notifications sent from client to server (fire-and-forget)
#[derive(Debug, Clone)]
pub enum ClientNotification {
    /// Server initialized notification (sent after initialize request)
    Initialized,
    /// Exit notification (sent after shutdown request)
    Exit,
    /// Document opened
    TextDocumentOpened(DidOpenTextDocumentParams),
    /// Document changed
    TextDocumentChanged(DidChangeTextDocumentParams),
    /// Document saved
    TextDocumentSaved(DidSaveTextDocumentParams),
}

impl ClientNotification {
    /// Convert to JSON-RPC method name and params
    pub fn to_json_rpc(&self) -> (&'static str, Value) {
        match self {
            ClientNotification::Initialized => ("initialized", Value::Object(Default::default())),
            ClientNotification::Exit => ("exit", Value::Null),
            ClientNotification::TextDocumentOpened(params) => (
                "textDocument/didOpen",
                to_value(params).unwrap_or(Value::Null),
            ),
            ClientNotification::TextDocumentChanged(params) => (
                "textDocument/didChange",
                to_value(params).unwrap_or(Value::Null),
            ),
            ClientNotification::TextDocumentSaved(params) => (
                "textDocument/didSave",
                to_value(params).unwrap_or(Value::Null),
            ),
        }
    }
}

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
    /// Parse the raw message into a typed `ParsedMessage`
    ///
    /// Returns `None` for messages that don't match expected patterns (e.g., requests from server).
    pub fn parse(self) -> Option<ParsedMessage> {
        // Check if this is a response (has id and result/error)
        if let Some(id) = self.id {
            if self.result.is_some() || self.error.is_some() {
                return Some(ParsedMessage::Response(ResponseMessage {
                    id,
                    result: self.result,
                    error: self.error,
                }));
            }
        }

        // Check if this is a notification (has method but no id)
        if let Some(method) = self.method {
            if self.id.is_none() {
                let notification = parse_notification(&method, self.params.unwrap_or(Value::Null));
                return Some(ParsedMessage::Notification(notification));
            }
        }

        None
    }
}

/// Parse a notification method and params into a typed `ParsedNotification`
fn parse_notification(method: &str, params: Value) -> ParsedNotification {
    match method {
        "textDocument/publishDiagnostics" => {
            match serde_json::from_value::<PublishDiagnosticsParams>(params) {
                Ok(diag_params) => ParsedNotification::Diagnostics(diag_params),
                Err(_) => ParsedNotification::Unknown(method.to_string()),
            }
        }
        "$/progress" => match serde_json::from_value::<ProgressParams>(params) {
            Ok(progress_params) => ParsedNotification::Progress(progress_params),
            Err(_) => ParsedNotification::Unknown(method.to_string()),
        },
        _ => ParsedNotification::Unknown(method.to_string()),
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
    fn test_incoming_message_parses_as_response() {
        let msg: IncomingMessage =
            serde_json::from_str(r#"{"jsonrpc":"2.0","id":1,"result":{"capabilities":{}}}"#)
                .unwrap();

        match msg.parse() {
            Some(ParsedMessage::Response(resp)) => {
                assert_eq!(resp.id, 1);
                assert!(resp.result.is_some());
                assert!(resp.error.is_none());
            }
            other => panic!("Expected Response, got {:?}", other),
        }
    }

    #[test]
    fn test_incoming_message_parses_as_diagnostics_notification() {
        let msg: IncomingMessage = serde_json::from_str(
            r#"{"jsonrpc":"2.0","method":"textDocument/publishDiagnostics","params":{"uri":"file:///test.rs","diagnostics":[]}}"#,
        )
        .unwrap();

        match msg.parse() {
            Some(ParsedMessage::Notification(ParsedNotification::Diagnostics(params))) => {
                assert_eq!(params.uri.as_str(), "file:///test.rs");
                assert!(params.diagnostics.is_empty());
            }
            other => panic!("Expected Diagnostics notification, got {:?}", other),
        }
    }

    #[test]
    fn test_incoming_message_parses_unknown_notification() {
        let msg: IncomingMessage = serde_json::from_str(
            r#"{"jsonrpc":"2.0","method":"window/logMessage","params":{"type":3,"message":"hello"}}"#,
        )
        .unwrap();

        match msg.parse() {
            Some(ParsedMessage::Notification(ParsedNotification::Unknown(method))) => {
                assert_eq!(method, "window/logMessage");
            }
            other => panic!("Expected Unknown notification, got {:?}", other),
        }
    }

    #[test]
    fn test_client_notification_to_json_rpc() {
        let (method, params) = ClientNotification::Initialized.to_json_rpc();
        assert_eq!(method, "initialized");
        assert!(params.is_object());

        let (method, _params) = ClientNotification::Exit.to_json_rpc();
        assert_eq!(method, "exit");
    }
}
