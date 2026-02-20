use lsp_types::{
    CallHierarchyIncomingCall, CallHierarchyIncomingCallsParams, CallHierarchyItem,
    CallHierarchyOutgoingCall, CallHierarchyOutgoingCallsParams, CallHierarchyPrepareParams,
    DidChangeTextDocumentParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams,
    DidSaveTextDocumentParams, DocumentSymbolParams, DocumentSymbolResponse, GotoDefinitionParams,
    GotoDefinitionResponse, Hover, HoverParams, Location, PublishDiagnosticsParams,
    ReferenceParams, SymbolInformation, Uri, WorkspaceSymbolParams,
};
use serde::{Deserialize, Serialize};
use std::io;
use std::path::PathBuf;

/// Language identifier for LSP
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, Serialize, Deserialize)]
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
    /// Get the LSP language ID string
    pub fn as_str(self) -> &'static str {
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
}

/// Top-level daemon request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DaemonRequest {
    Initialize(InitializeRequest),
    LspRequest(LspRequest),
    LspNotification(LspNotification),
    Disconnect,
    Ping,
}

/// Initialize request to set up LSP for a workspace
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitializeRequest {
    pub workspace_root: PathBuf,
    pub language: LanguageId,
}

/// LSP request with client ID for response correlation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LspRequest {
    GotoDefinition {
        client_id: i64,
        params: GotoDefinitionParams,
    },
    GotoImplementation {
        client_id: i64,
        params: GotoDefinitionParams,
    },
    FindReferences {
        client_id: i64,
        params: ReferenceParams,
    },
    Hover {
        client_id: i64,
        params: HoverParams,
    },
    WorkspaceSymbol {
        client_id: i64,
        params: WorkspaceSymbolParams,
    },
    DocumentSymbol {
        client_id: i64,
        params: DocumentSymbolParams,
    },
    PrepareCallHierarchy {
        client_id: i64,
        params: CallHierarchyPrepareParams,
    },
    IncomingCalls {
        client_id: i64,
        params: CallHierarchyIncomingCallsParams,
    },
    OutgoingCalls {
        client_id: i64,
        params: CallHierarchyOutgoingCallsParams,
    },
    /// Get cached diagnostics for a file or all files
    GetDiagnostics {
        client_id: i64,
        /// If None, return all cached diagnostics for the workspace
        uri: Option<Uri>,
    },
}

impl LspRequest {
    /// Get the client ID from the request
    pub fn client_id(&self) -> i64 {
        match self {
            LspRequest::GotoDefinition { client_id, .. }
            | LspRequest::GotoImplementation { client_id, .. }
            | LspRequest::FindReferences { client_id, .. }
            | LspRequest::Hover { client_id, .. }
            | LspRequest::WorkspaceSymbol { client_id, .. }
            | LspRequest::DocumentSymbol { client_id, .. }
            | LspRequest::PrepareCallHierarchy { client_id, .. }
            | LspRequest::IncomingCalls { client_id, .. }
            | LspRequest::OutgoingCalls { client_id, .. }
            | LspRequest::GetDiagnostics { client_id, .. } => *client_id,
        }
    }
}

/// LSP notification from client to server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LspNotification {
    Opened(DidOpenTextDocumentParams),
    Changed(DidChangeTextDocumentParams),
    Saved(DidSaveTextDocumentParams),
    Closed(DidCloseTextDocumentParams),
}

/// Top-level daemon response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DaemonResponse {
    Initialized,
    Pong,
    LspResponse(LspResponse),
    Error(ProtocolError),
}

/// LSP response with client ID for correlation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LspResponse {
    GotoDefinition {
        client_id: i64,
        result: Result<GotoDefinitionResponse, LspErrorResponse>,
    },
    GotoImplementation {
        client_id: i64,
        result: Result<GotoDefinitionResponse, LspErrorResponse>,
    },
    FindReferences {
        client_id: i64,
        result: Result<Vec<Location>, LspErrorResponse>,
    },
    Hover {
        client_id: i64,
        result: Result<Option<Hover>, LspErrorResponse>,
    },
    WorkspaceSymbol {
        client_id: i64,
        result: Result<Vec<SymbolInformation>, LspErrorResponse>,
    },
    DocumentSymbol {
        client_id: i64,
        result: Result<DocumentSymbolResponse, LspErrorResponse>,
    },
    PrepareCallHierarchy {
        client_id: i64,
        result: Result<Vec<CallHierarchyItem>, LspErrorResponse>,
    },
    IncomingCalls {
        client_id: i64,
        result: Result<Vec<CallHierarchyIncomingCall>, LspErrorResponse>,
    },
    OutgoingCalls {
        client_id: i64,
        result: Result<Vec<CallHierarchyOutgoingCall>, LspErrorResponse>,
    },
    GetDiagnostics {
        client_id: i64,
        result: Result<Vec<PublishDiagnosticsParams>, LspErrorResponse>,
    },
}

/// LSP error response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LspErrorResponse {
    pub code: i32,
    pub message: String,
}

/// Protocol-level error (not LSP error)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolError {
    pub message: String,
    /// Optional `client_id` for correlating errors back to LSP requests
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_id: Option<i64>,
}

impl ProtocolError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            client_id: None,
        }
    }

    pub fn with_client_id(message: impl Into<String>, client_id: i64) -> Self {
        Self {
            message: message.into(),
            client_id: Some(client_id),
        }
    }
}

/// Maximum message size (16 MB)
pub const MAX_MESSAGE_SIZE: u32 = 16 * 1024 * 1024;

/// Read a length-prefixed frame from an async reader
pub async fn read_frame<R, T>(reader: &mut R) -> io::Result<Option<T>>
where
    R: tokio::io::AsyncReadExt + Unpin,
    T: for<'de> Deserialize<'de>,
{
    let mut len_buf = [0u8; 4];
    match reader.read_exact(&mut len_buf).await {
        Ok(_) => {}
        Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(e),
    }

    let len = u32::from_be_bytes(len_buf);

    if len > MAX_MESSAGE_SIZE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Message too large: {len} bytes"),
        ));
    }

    let mut buf = vec![0u8; len as usize];
    reader.read_exact(&mut buf).await?;

    serde_json::from_slice(&buf)
        .map(Some)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

/// Write a length-prefixed frame to an async writer
pub async fn write_frame<W, T>(writer: &mut W, message: &T) -> io::Result<()>
where
    W: tokio::io::AsyncWriteExt + Unpin,
    T: Serialize,
{
    let json = serde_json::to_vec(message)?;

    if json.len() > MAX_MESSAGE_SIZE as usize {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Message too large: {} bytes", json.len()),
        ));
    }

    let len = u32::try_from(json.len()).unwrap_or(u32::MAX);
    writer.write_all(&len.to_be_bytes()).await?;
    writer.write_all(&json).await?;
    writer.flush().await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_language_id_as_str() {
        assert_eq!(LanguageId::Rust.as_str(), "rust");
        assert_eq!(LanguageId::TypeScriptReact.as_str(), "typescriptreact");
    }

    #[test]
    fn test_protocol_error_new() {
        let err = ProtocolError::new("test error");
        assert_eq!(err.message, "test error");
    }

    #[test]
    fn test_lsp_request_client_id() {
        let req = LspRequest::GotoDefinition {
            client_id: 42,
            params: GotoDefinitionParams {
                text_document_position_params: lsp_types::TextDocumentPositionParams {
                    text_document: lsp_types::TextDocumentIdentifier {
                        uri: "file:///test.rs".parse().unwrap(),
                    },
                    position: lsp_types::Position {
                        line: 0,
                        character: 0,
                    },
                },
                work_done_progress_params: Default::default(),
                partial_result_params: Default::default(),
            },
        };
        assert_eq!(req.client_id(), 42);
    }
}
