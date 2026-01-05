//! Platform-agnostic resource building for file attachments.
//!
//! Provides unified ContentBlock builders that work across desktop and web.

use crate::file_types::FileMatch;
use agent_client_protocol::{ContentBlock, ResourceLink};
use std::path::Path;

#[cfg(feature = "desktop")]
use agent_client_protocol::{EmbeddedResource, EmbeddedResourceResource, TextResourceContents};

/// Build a ContentBlock for a file attachment.
///
/// Desktop with embedded support: Returns `EmbeddedResource` with file content.
/// Desktop without embedded support: Returns `ResourceLink`.
/// Web: Always returns `ResourceLink` (no file content access).
#[cfg(feature = "desktop")]
pub fn file_to_content_block(
    file: &FileMatch,
    content: Option<&str>,
    supports_embedded: bool,
) -> ContentBlock {
    if supports_embedded {
        if let Some(content) = content {
            ContentBlock::Resource(EmbeddedResource {
                annotations: None,
                resource: EmbeddedResourceResource::TextResourceContents(TextResourceContents {
                    uri: format!("file://{}", file.absolute_path.display()),
                    text: content.to_string(),
                    mime_type: mime_from_path(&file.path),
                    meta: None,
                }),
                meta: None,
            })
        } else {
            ContentBlock::Resource(EmbeddedResource {
                annotations: None,
                resource: EmbeddedResourceResource::TextResourceContents(TextResourceContents {
                    uri: format!("file://{}", file.absolute_path.display()),
                    text: String::new(),
                    mime_type: mime_from_path(&file.path),
                    meta: None,
                }),
                meta: None,
            })
        }
    } else {
        file_to_resource_link(file)
    }
}

#[cfg(not(feature = "desktop"))]
pub fn file_to_content_block(
    file: &FileMatch,
    _content: Option<&str>,
    _supports_embedded: bool,
) -> ContentBlock {
    file_to_resource_link(file)
}

/// Build a ContentBlock::ResourceLink for a file reference.
pub fn file_to_resource_link(file: &FileMatch) -> ContentBlock {
    ContentBlock::ResourceLink(ResourceLink {
        uri: format!("file://{}", file.absolute_path.display()),
        name: file.path.clone(),
        size: Some(file.size as i64),
        mime_type: mime_from_path(&file.path),
        title: Some(file.path.clone()),
        description: None,
        annotations: None,
        meta: None,
    })
}

/// Infer MIME type from file extension.
pub fn mime_from_path(path: &str) -> Option<String> {
    let ext = Path::new(path).extension()?.to_str()?;
    let mime = match ext.to_lowercase().as_str() {
        "rs" => "text/x-rust",
        "py" => "text/x-python",
        "js" => "text/javascript",
        "ts" => "text/typescript",
        "tsx" => "text/typescript-jsx",
        "jsx" => "text/javascript-jsx",
        "json" => "application/json",
        "toml" => "text/x-toml",
        "yaml" | "yml" => "text/x-yaml",
        "md" => "text/markdown",
        "html" => "text/html",
        "css" => "text/css",
        "go" => "text/x-go",
        "java" => "text/x-java",
        "c" => "text/x-c",
        "cpp" | "cc" | "cxx" => "text/x-c++",
        "h" | "hpp" => "text/x-c-header",
        "sh" | "bash" => "text/x-shellscript",
        "sql" => "text/x-sql",
        "xml" => "text/xml",
        "txt" => "text/plain",
        _ => "text/plain",
    };
    Some(mime.to_string())
}
