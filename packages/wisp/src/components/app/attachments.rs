use super::PromptAttachment;
use agent_client_protocol as acp;
use std::path::Path;
use tokio::io::AsyncReadExt;
use url::Url;

const MAX_EMBED_TEXT_BYTES: usize = 1024 * 1024;

#[derive(Debug, Default)]
pub struct AttachmentBuildOutcome {
    pub blocks: Vec<acp::ContentBlock>,
    pub warnings: Vec<String>,
}

pub async fn build_attachment_blocks(attachments: &[PromptAttachment]) -> AttachmentBuildOutcome {
    let mut outcome = AttachmentBuildOutcome::default();

    for attachment in attachments {
        match try_build_attachment_block(&attachment.path, &attachment.display_name).await {
            Ok((block, maybe_warning)) => {
                outcome.blocks.push(block);
                if let Some(warning) = maybe_warning {
                    outcome.warnings.push(warning);
                }
            }
            Err(warning) => outcome.warnings.push(warning),
        }
    }

    outcome
}

async fn try_build_attachment_block(
    path: &Path,
    display_name: &str,
) -> Result<(acp::ContentBlock, Option<String>), String> {
    let file = tokio::fs::File::open(path)
        .await
        .map_err(|error| format!("Failed to read {display_name}: {error}"))?;

    let mut bytes = Vec::new();
    file.take((MAX_EMBED_TEXT_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .await
        .map_err(|error| format!("Failed to read {display_name}: {error}"))?;

    let truncated = bytes.len() > MAX_EMBED_TEXT_BYTES;
    if truncated {
        bytes.truncate(MAX_EMBED_TEXT_BYTES);
    }
    let text_bytes = bytes.as_slice();

    let text = match std::str::from_utf8(text_bytes) {
        Ok(text) => text.to_string(),
        Err(error) if truncated && error.valid_up_to() > 0 => {
            let valid_bytes = &text_bytes[..error.valid_up_to()];
            std::str::from_utf8(valid_bytes)
                .expect("valid_up_to must point at a utf8 boundary")
                .to_string()
        }
        Err(_) => return Err(format!("Skipped binary or non-UTF8 file: {display_name}")),
    };

    let file_uri = build_attachment_file_uri(path, display_name).await?;
    let mime_type = mime_guess::from_path(path)
        .first_or_octet_stream()
        .to_string();
    let warning =
        truncated.then(|| format!("Truncated {display_name} to {MAX_EMBED_TEXT_BYTES} bytes"));

    let block = acp::ContentBlock::Resource(acp::EmbeddedResource::new(
        acp::EmbeddedResourceResource::TextResourceContents(
            acp::TextResourceContents::new(text, file_uri).mime_type(mime_type),
        ),
    ));

    Ok((block, warning))
}

async fn build_attachment_file_uri(path: &Path, display_name: &str) -> Result<String, String> {
    let canonical_path = tokio::fs::canonicalize(path).await.ok();
    let uri_path = canonical_path.as_deref().unwrap_or(path);
    Url::from_file_path(uri_path)
        .map_err(|()| format!("Failed to build file URI for {display_name}"))
        .map(|url| url.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn build_attachment_blocks_truncates_large_file_with_warning() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("large.txt");
        let display_name = "large.txt".to_string();
        std::fs::write(&path, "x".repeat(MAX_EMBED_TEXT_BYTES + 64)).unwrap();

        let attachments = vec![PromptAttachment {
            path,
            display_name: display_name.clone(),
        }];
        let outcome = build_attachment_blocks(&attachments).await;

        assert_eq!(outcome.blocks.len(), 1);
        assert_eq!(outcome.warnings.len(), 1);
        assert!(outcome.warnings[0].contains(&format!(
            "Truncated {display_name} to {MAX_EMBED_TEXT_BYTES} bytes"
        )));
    }

    #[tokio::test]
    async fn build_attachment_blocks_skips_non_utf8_files() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("binary.bin");
        let display_name = "binary.bin".to_string();
        std::fs::write(&path, [0xff, 0xfe, 0xfd]).unwrap();

        let attachments = vec![PromptAttachment {
            path,
            display_name: display_name.clone(),
        }];
        let outcome = build_attachment_blocks(&attachments).await;

        assert!(outcome.blocks.is_empty());
        assert_eq!(outcome.warnings.len(), 1);
        assert!(
            outcome.warnings[0]
                .contains(&format!("Skipped binary or non-UTF8 file: {display_name}"))
        );
    }

    #[tokio::test]
    async fn build_attachment_file_uri_falls_back_when_canonicalize_fails() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("missing.txt");
        let expected = Url::from_file_path(&path).unwrap().to_string();

        let uri = build_attachment_file_uri(&path, "missing.txt")
            .await
            .expect("URI should be built from original absolute path");

        assert_eq!(uri, expected);
    }
}
