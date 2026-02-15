use agent_client_protocol as acp;
use mcp_coding::CodingTools;
use mcp_coding::display_meta::{ToolDisplayMeta, truncate};
use mcp_coding::error::{BashError, CodingError, FileError};
use mcp_coding::tools::bash::{
    BackgroundProcessHandle, BashInput, BashOutput, BashResult, ReadBackgroundBashOutput,
};
use mcp_coding::tools::edit_file::{EditFileArgs, EditFileResponse};
use mcp_coding::tools::list_files::{ListFilesArgs, ListFilesResult};
use mcp_coding::tools::read_file::{ReadFileArgs, ReadFileResult};
use mcp_coding::tools::write_file::{WriteFileArgs, WriteFileResponse};
use std::collections::HashMap;
use std::sync::Mutex;
use tracing::{debug, warn};

use crate::acp_actor::AcpActorHandle;

/// Implementation of CodingTools that delegates to ACP client methods via an actor.
///
/// This allows the LLM's file/terminal operations to be routed through
/// the editor (ACP client), enabling the editor to track and control
/// what the agent is doing.
///
/// The actor pattern solves the Send/Sync problem:
/// - AcpActorHandle uses channels, which are Send+Sync
/// - The actual ACP connection lives in an actor on a LocalSet
#[derive(Debug, Clone)]
pub struct AcpCodingTools {
    actor_handle: AcpActorHandle,
    session_id: acp::SessionId,
    /// Map background bash shell IDs to ACP terminal IDs
    terminal_map: std::sync::Arc<Mutex<HashMap<String, acp::TerminalId>>>,
}

impl AcpCodingTools {
    pub fn new(actor_handle: AcpActorHandle, session_id: acp::SessionId) -> Self {
        Self {
            actor_handle,
            session_id,
            terminal_map: std::sync::Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl CodingTools for AcpCodingTools {
    async fn read_file(&self, args: ReadFileArgs) -> Result<ReadFileResult, CodingError> {
        debug!("ACP read_file: {}", args.file_path);

        let mut request =
            acp::ReadTextFileRequest::new(self.session_id.clone(), args.file_path.clone());
        if let Some(offset) = args.offset {
            request = request.line(offset as u32);
        }
        if let Some(limit) = args.limit {
            request = request.limit(limit as u32);
        }

        let response = self
            .actor_handle
            .read_text_file(request)
            .await
            .map_err(|e| FileError::ReadFailed {
                path: args.file_path.clone(),
                reason: e,
            })?;

        // Parse the content to add line numbers similar to read_file_contents format
        let lines: Vec<&str> = response.content.lines().collect();
        let start_line = args.offset.unwrap_or(1);

        let formatted_content = lines
            .iter()
            .enumerate()
            .map(|(i, line)| {
                let line_num = start_line + i;
                format!("{line_num:>6}\t{line}")
            })
            .collect::<Vec<_>>()
            .join("\n");

        // Calculate the total lines and lines shown
        let total_lines = response.content.lines().count();
        let lines_shown = lines.len();
        let offset = args.offset.unwrap_or(1);

        let display_meta = ToolDisplayMeta::read_file(
            args.file_path.clone(),
            Some(response.content.len()),
            Some(total_lines),
        );

        Ok(ReadFileResult {
            status: "success".to_string(),
            file_path: args.file_path.clone(),
            content: formatted_content,
            total_lines,
            lines_shown,
            offset,
            limit: args.limit,
            size: response.content.len(),
            raw_content: response.content,
            _meta: display_meta.into_meta(),
        })
    }

    async fn write_file(&self, args: WriteFileArgs) -> Result<WriteFileResponse, CodingError> {
        debug!("ACP write_file: {}", args.file_path);

        let bytes_written = args.content.len();

        self.actor_handle
            .write_text_file(acp::WriteTextFileRequest::new(
                self.session_id.clone(),
                args.file_path.clone(),
                args.content,
            ))
            .await
            .map_err(|e| FileError::WriteFailed {
                path: args.file_path.clone(),
                reason: e,
            })?;

        let display_meta = ToolDisplayMeta::write_file(args.file_path.clone(), Some(bytes_written));

        Ok(WriteFileResponse {
            message: format!("Successfully wrote to {}", args.file_path),
            bytes_written,
            file_path: args.file_path,
            _meta: display_meta.into_meta(),
        })
    }

    async fn edit_file(&self, args: EditFileArgs) -> Result<EditFileResponse, CodingError> {
        debug!("ACP edit_file: {} (via read+write)", args.file_path);

        // ACP doesn't have a native "edit" operation, so we:
        // 1. Read the file
        // 2. Perform the replacement
        // 3. Write it back

        let read_response = self
            .actor_handle
            .read_text_file(acp::ReadTextFileRequest::new(
                self.session_id.clone(),
                args.file_path.clone(),
            ))
            .await
            .map_err(|e| FileError::ReadFailed {
                path: args.file_path.clone(),
                reason: e,
            })?;

        let content = read_response.content;

        // Perform the replacement
        let (new_content, replacements_made) = if args.replace_all {
            let count = content.matches(&args.old_string).count();
            (content.replace(&args.old_string, &args.new_string), count)
        } else {
            // Single replacement - check uniqueness
            let count = content.matches(&args.old_string).count();
            if count == 0 {
                return Err(FileError::PatternNotFound {
                    path: args.file_path.clone(),
                    pattern: args.old_string.clone(),
                }
                .into());
            } else if count > 1 {
                return Err(CodingError::NotConfigured(format!(
                    "String appears {count} times in file. Use replace_all=true or provide more context to make it unique."
                )));
            }
            (content.replacen(&args.old_string, &args.new_string, 1), 1)
        };

        // Write back
        self.actor_handle
            .write_text_file(acp::WriteTextFileRequest::new(
                self.session_id.clone(),
                args.file_path.clone(),
                new_content.clone(),
            ))
            .await
            .map_err(|e| FileError::WriteFailed {
                path: args.file_path.clone(),
                reason: e,
            })?;

        let total_lines = new_content.lines().count();

        let display_meta = ToolDisplayMeta::edit_file(
            args.file_path.clone(),
            Some(truncate(&args.old_string, 100)),
            Some(truncate(&args.new_string, 100)),
        );

        Ok(EditFileResponse {
            status: "success".to_string(),
            file_path: args.file_path,
            total_lines,
            replacements_made,
            content: new_content,
            _meta: display_meta.into_meta(),
        })
    }

    async fn list_files(&self, args: ListFilesArgs) -> Result<ListFilesResult, CodingError> {
        debug!("ACP list_files: {:?}", args.path);

        // ACP doesn't have a list_files method, so fall back to local filesystem
        // This is acceptable since listing is read-only and doesn't modify state
        warn!("ACP doesn't support list_files, falling back to local filesystem");

        // Use the default implementation
        mcp_coding::tools::list_files::list_files(args)
            .await
            .map_err(CodingError::from)
    }

    async fn bash(&self, args: BashInput) -> Result<BashResult, CodingError> {
        debug!("ACP bash: {}", args.command);

        let response = self
            .actor_handle
            .create_terminal(acp::CreateTerminalRequest::new(
                self.session_id.clone(),
                args.command.clone(),
            ))
            .await
            .map_err(|e| BashError::SpawnFailed {
                command: args.command.clone(),
                reason: e,
            })?;

        let terminal_id = response.terminal_id;

        if args.run_in_background.unwrap_or(false) {
            // Generate a shell_id and map it to the terminal_id
            let shell_id = format!("acp-terminal-{terminal_id}");
            self.terminal_map
                .lock()
                .unwrap()
                .insert(shell_id.clone(), terminal_id);

            // Create a dummy channel and task for the BackgroundProcessHandle
            // since ACP terminals don't use local processes
            let (_tx, output_rx) = tokio::sync::mpsc::unbounded_channel();
            let task_handle = tokio::spawn(async {
                futures::future::pending::<()>().await;
                (0, false) // exit_code, killed
            });

            Ok(BashResult::Background(BackgroundProcessHandle {
                shell_id,
                output_rx,
                task_handle,
            }))
        } else {
            // Wait for the terminal to exit
            let exit_response = self
                .actor_handle
                .wait_for_terminal_exit(acp::WaitForTerminalExitRequest::new(
                    self.session_id.clone(),
                    terminal_id.clone(),
                ))
                .await
                .map_err(BashError::WaitFailed)?;

            // Get the output
            let output_response = self
                .actor_handle
                .terminal_output(acp::TerminalOutputRequest::new(
                    self.session_id.clone(),
                    terminal_id.clone(),
                ))
                .await
                .map_err(BashError::WaitFailed)?;

            // Release the terminal
            let _ = self
                .actor_handle
                .release_terminal(acp::ReleaseTerminalRequest::new(
                    self.session_id.clone(),
                    terminal_id,
                ))
                .await;

            let exit_code = exit_response.exit_status.exit_code.unwrap_or(0) as i32;
            let display_meta = ToolDisplayMeta::command(
                args.command.clone(),
                args.description,
                exit_code,
                Some(false),
            );

            Ok(BashResult::Completed(BashOutput {
                output: output_response.output,
                exit_code,
                killed: None,
                shell_id: None,
                _meta: display_meta.into_meta(),
            }))
        }
    }

    async fn read_background_bash(
        &self,
        handle: BackgroundProcessHandle,
        _filter: Option<String>,
    ) -> Result<(ReadBackgroundBashOutput, Option<BackgroundProcessHandle>), CodingError> {
        debug!("ACP read_background_bash: {}", handle.shell_id);

        // Look up the terminal_id
        let terminal_id = self
            .terminal_map
            .lock()
            .unwrap()
            .get(&handle.shell_id)
            .cloned()
            .ok_or_else(|| BashError::ShellNotFound(handle.shell_id.clone()))?;

        // Get output
        let output_response = self
            .actor_handle
            .terminal_output(acp::TerminalOutputRequest::new(
                self.session_id.clone(),
                terminal_id.clone(),
            ))
            .await
            .map_err(BashError::WaitFailed)?;

        let is_running = output_response.exit_status.is_none();
        let exit_code = output_response
            .exit_status
            .as_ref()
            .and_then(|s| s.exit_code)
            .map(|c| c as i32);

        let description = if is_running {
            "Running in background"
        } else {
            "Background process completed"
        };

        let display_meta = ToolDisplayMeta::command(
            format!("ACP terminal {}", terminal_id),
            Some(description.to_string()),
            exit_code.unwrap_or(0),
            Some(false),
        );

        let result = ReadBackgroundBashOutput {
            output: output_response.output,
            status: if is_running {
                "running".to_string()
            } else {
                "completed".to_string()
            },
            exit_code,
            _meta: display_meta.into_meta(),
        };

        if is_running {
            // Still running, return the handle
            Ok((result, Some(handle)))
        } else {
            // Completed, release the terminal and remove from map
            let _ = self
                .actor_handle
                .release_terminal(acp::ReleaseTerminalRequest::new(
                    self.session_id.clone(),
                    terminal_id,
                ))
                .await;

            self.terminal_map.lock().unwrap().remove(&handle.shell_id);

            Ok((result, None))
        }
    }
}
