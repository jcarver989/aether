use super::error::{LspError, Result};
use crate::coding::file_types::lsp_id_from_path;
use lsp_types::{
    CallHierarchyIncomingCallsParams, CallHierarchyOutgoingCallsParams, CallHierarchyPrepareParams,
    DidChangeTextDocumentParams, DidOpenTextDocumentParams, DidSaveTextDocumentParams,
    DocumentSymbolParams, GotoDefinitionParams, HoverParams, InitializeParams, ProgressParams,
    PublishDiagnosticsParams, ReferenceParams, WorkspaceSymbolParams,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, to_value};
use std::path::Path;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::{ChildStdin, ChildStdout};

/// LSP language identifier for a file type
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum LanguageId {
    Rust,
    Python,
    JavaScript,
    JavaScriptReact,
    TypeScript,
    TypeScriptReact,
    Go,
    Java,
    C,
    Cpp,
    CSharp,
    Ruby,
    Php,
    Swift,
    Kotlin,
    Scala,
    Html,
    Css,
    Json,
    Yaml,
    Toml,
    Markdown,
    Xml,
    Sql,
    ShellScript,
    PlainText,
}

impl LanguageId {
    /// Get the LSP language ID string for this language
    pub fn as_str(&self) -> &'static str {
        match self {
            LanguageId::Rust => "rust",
            LanguageId::Python => "python",
            LanguageId::JavaScript => "javascript",
            LanguageId::JavaScriptReact => "javascriptreact",
            LanguageId::TypeScript => "typescript",
            LanguageId::TypeScriptReact => "typescriptreact",
            LanguageId::Go => "go",
            LanguageId::Java => "java",
            LanguageId::C => "c",
            LanguageId::Cpp => "cpp",
            LanguageId::CSharp => "csharp",
            LanguageId::Ruby => "ruby",
            LanguageId::Php => "php",
            LanguageId::Swift => "swift",
            LanguageId::Kotlin => "kotlin",
            LanguageId::Scala => "scala",
            LanguageId::Html => "html",
            LanguageId::Css => "css",
            LanguageId::Json => "json",
            LanguageId::Yaml => "yaml",
            LanguageId::Toml => "toml",
            LanguageId::Markdown => "markdown",
            LanguageId::Xml => "xml",
            LanguageId::Sql => "sql",
            LanguageId::ShellScript => "shellscript",
            LanguageId::PlainText => "plaintext",
        }
    }

    /// Detect language ID from file path
    pub fn from_path(path: &Path) -> Self {
        Self::from_lsp_id(lsp_id_from_path(path))
    }

    /// Convert an LSP language ID string to a LanguageId enum variant
    fn from_lsp_id(lsp_id: &str) -> Self {
        match lsp_id {
            "rust" => LanguageId::Rust,
            "python" => LanguageId::Python,
            "javascript" => LanguageId::JavaScript,
            "javascriptreact" => LanguageId::JavaScriptReact,
            "typescript" => LanguageId::TypeScript,
            "typescriptreact" => LanguageId::TypeScriptReact,
            "go" => LanguageId::Go,
            "java" => LanguageId::Java,
            "c" => LanguageId::C,
            "cpp" => LanguageId::Cpp,
            "csharp" => LanguageId::CSharp,
            "ruby" => LanguageId::Ruby,
            "php" => LanguageId::Php,
            "swift" => LanguageId::Swift,
            "kotlin" => LanguageId::Kotlin,
            "scala" => LanguageId::Scala,
            "html" => LanguageId::Html,
            "css" => LanguageId::Css,
            "json" => LanguageId::Json,
            "yaml" => LanguageId::Yaml,
            "toml" => LanguageId::Toml,
            "markdown" => LanguageId::Markdown,
            "xml" => LanguageId::Xml,
            "sql" => LanguageId::Sql,
            "shellscript" => LanguageId::ShellScript,
            _ => LanguageId::PlainText,
        }
    }

    /// Get the primary file extension for this language (reverse of from_path)
    ///
    /// Returns None for PlainText since it has no specific extension.
    pub fn extension(&self) -> Option<&'static str> {
        match self {
            LanguageId::Rust => Some("rs"),
            LanguageId::Python => Some("py"),
            LanguageId::JavaScript => Some("js"),
            LanguageId::JavaScriptReact => Some("jsx"),
            LanguageId::TypeScript => Some("ts"),
            LanguageId::TypeScriptReact => Some("tsx"),
            LanguageId::Go => Some("go"),
            LanguageId::Java => Some("java"),
            LanguageId::C => Some("c"),
            LanguageId::Cpp => Some("cpp"),
            LanguageId::CSharp => Some("cs"),
            LanguageId::Ruby => Some("rb"),
            LanguageId::Php => Some("php"),
            LanguageId::Swift => Some("swift"),
            LanguageId::Kotlin => Some("kt"),
            LanguageId::Scala => Some("scala"),
            LanguageId::Html => Some("html"),
            LanguageId::Css => Some("css"),
            LanguageId::Json => Some("json"),
            LanguageId::Yaml => Some("yaml"),
            LanguageId::Toml => Some("toml"),
            LanguageId::Markdown => Some("md"),
            LanguageId::Xml => Some("xml"),
            LanguageId::Sql => Some("sql"),
            LanguageId::ShellScript => Some("sh"),
            LanguageId::PlainText => None,
        }
    }
}

impl std::fmt::Display for LanguageId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

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
    fn to_json_rpc(&self) -> (&'static str, Value) {
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

/// Requests sent from client to server (expects a response)
#[derive(Debug, Clone)]
pub enum ClientRequest {
    /// Initialize the language server
    Initialize(i64, Box<InitializeParams>),
    /// Shutdown the language server
    Shutdown(i64),
    /// Go to definition
    GotoDefinition(i64, GotoDefinitionParams),
    /// Go to implementation
    GotoImplementation(i64, GotoDefinitionParams),
    /// Find references
    FindReferences(i64, ReferenceParams),
    /// Hover (get type/documentation info)
    Hover(i64, HoverParams),
    /// Workspace symbol search
    WorkspaceSymbol(i64, WorkspaceSymbolParams),
    /// Document symbol (get symbols in a document)
    DocumentSymbol(i64, DocumentSymbolParams),
    /// Prepare call hierarchy
    PrepareCallHierarchy(i64, CallHierarchyPrepareParams),
    /// Incoming calls
    IncomingCalls(i64, CallHierarchyIncomingCallsParams),
    /// Outgoing calls
    OutgoingCalls(i64, CallHierarchyOutgoingCallsParams),
}

impl ClientRequest {
    /// Get the request ID
    pub fn id(&self) -> i64 {
        match self {
            ClientRequest::Initialize(id, _) => *id,
            ClientRequest::Shutdown(id) => *id,
            ClientRequest::GotoDefinition(id, _) => *id,
            ClientRequest::GotoImplementation(id, _) => *id,
            ClientRequest::FindReferences(id, _) => *id,
            ClientRequest::Hover(id, _) => *id,
            ClientRequest::WorkspaceSymbol(id, _) => *id,
            ClientRequest::DocumentSymbol(id, _) => *id,
            ClientRequest::PrepareCallHierarchy(id, _) => *id,
            ClientRequest::IncomingCalls(id, _) => *id,
            ClientRequest::OutgoingCalls(id, _) => *id,
        }
    }
}

/// A JSON-RPC request message
#[derive(Debug, Serialize)]
struct JsonRpcRequest<P: Serialize> {
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
struct JsonRpcNotification<P: Serialize> {
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
struct IncomingMessage {
    #[allow(dead_code)]
    pub jsonrpc: String,
    pub id: Option<i64>,
    pub method: Option<String>,
    pub params: Option<Value>,
    pub result: Option<Value>,
    pub error: Option<JsonRpcError>,
}

impl From<IncomingMessage> for Option<ParsedMessage> {
    fn from(msg: IncomingMessage) -> Self {
        // Check if this is a response (has id and result/error)
        if let Some(id) = msg.id
            && (msg.result.is_some() || msg.error.is_some())
        {
            return Some(ParsedMessage::Response(ResponseMessage {
                id,
                result: msg.result,
                error: msg.error,
            }));
        }

        // Check if this is a notification (has method but no id)
        if let Some(method) = msg.method
            && msg.id.is_none()
        {
            let notification = parse_notification(&method, msg.params.unwrap_or(Value::Null));
            return Some(ParsedMessage::Notification(notification));
        }

        None
    }
}

/// Read and parse a single LSP message from the server's stdout
///
/// Returns `None` for unknown message types (e.g., server requests).
pub async fn read_message(reader: &mut BufReader<ChildStdout>) -> Result<Option<ParsedMessage>> {
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
    Ok(message.into())
}

/// Send a client request to the server
pub async fn send_request(writer: &mut ChildStdin, request: &ClientRequest) -> Result<()> {
    let (id, method, params) = match request {
        ClientRequest::Initialize(id, params) => {
            (*id, "initialize", to_value(params).unwrap_or(Value::Null))
        }
        ClientRequest::Shutdown(id) => (*id, "shutdown", Value::Null),
        ClientRequest::GotoDefinition(id, params) => (
            *id,
            "textDocument/definition",
            to_value(params).unwrap_or(Value::Null),
        ),
        ClientRequest::GotoImplementation(id, params) => (
            *id,
            "textDocument/implementation",
            to_value(params).unwrap_or(Value::Null),
        ),
        ClientRequest::FindReferences(id, params) => (
            *id,
            "textDocument/references",
            to_value(params).unwrap_or(Value::Null),
        ),
        ClientRequest::Hover(id, params) => (
            *id,
            "textDocument/hover",
            to_value(params).unwrap_or(Value::Null),
        ),
        ClientRequest::WorkspaceSymbol(id, params) => (
            *id,
            "workspace/symbol",
            to_value(params).unwrap_or(Value::Null),
        ),
        ClientRequest::DocumentSymbol(id, params) => (
            *id,
            "textDocument/documentSymbol",
            to_value(params).unwrap_or(Value::Null),
        ),
        ClientRequest::PrepareCallHierarchy(id, params) => (
            *id,
            "textDocument/prepareCallHierarchy",
            to_value(params).unwrap_or(Value::Null),
        ),
        ClientRequest::IncomingCalls(id, params) => (
            *id,
            "callHierarchy/incomingCalls",
            to_value(params).unwrap_or(Value::Null),
        ),
        ClientRequest::OutgoingCalls(id, params) => (
            *id,
            "callHierarchy/outgoingCalls",
            to_value(params).unwrap_or(Value::Null),
        ),
    };

    let json_request = JsonRpcRequest::new(id, method, params);
    let content = serde_json::to_string(&json_request)?;
    write_raw_message(writer, &content).await
}

/// Send a client notification to the server
pub async fn send_notification(
    writer: &mut ChildStdin,
    notification: &ClientNotification,
) -> Result<()> {
    let (method, params) = notification.to_json_rpc();
    let json_notification = JsonRpcNotification::new(method, params);
    let content = serde_json::to_string(&json_notification)?;
    write_raw_message(writer, &content).await
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

        let parsed: Option<ParsedMessage> = msg.into();
        match parsed {
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

        let parsed: Option<ParsedMessage> = msg.into();
        match parsed {
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

        let parsed: Option<ParsedMessage> = msg.into();
        match parsed {
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
