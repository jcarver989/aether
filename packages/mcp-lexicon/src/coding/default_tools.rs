use super::{
    BackgroundProcessHandle, BashInput, BashResult, EditFileArgs, EditFileResponse, ListFilesArgs,
    ListFilesResult, ReadBackgroundBashOutput, ReadFileArgs, ReadFileResult, WriteFileArgs,
    WriteFileResponse, edit_file_contents, execute_command, list_files, read_background_bash,
    read_file_contents, tools_trait::CodingTools, write_file_contents,
};

/// Default implementation that uses local filesystem operations.
///
/// This is the standard behavior for CodingMcp when running outside
/// of an ACP context.
#[derive(Debug, Default, Clone)]
pub struct DefaultCodingTools;

impl CodingTools for DefaultCodingTools {
    fn read_file(&self, args: ReadFileArgs) -> impl Future<Output = Result<ReadFileResult, String>> + Send {
        async move {
            read_file_contents(args)
                .await
                .map_err(|e| format!("Read file error: {e}"))
        }
    }

    fn write_file(&self, args: WriteFileArgs) -> impl Future<Output = Result<WriteFileResponse, String>> + Send {
        async move {
            write_file_contents(args)
                .await
                .map_err(|e| format!("Write file error: {e}"))
        }
    }

    fn edit_file(&self, args: EditFileArgs) -> impl Future<Output = Result<EditFileResponse, String>> + Send {
        async move {
            edit_file_contents(args)
                .await
                .map_err(|e| format!("Edit file error: {e}"))
        }
    }

    fn list_files(&self, args: ListFilesArgs) -> impl Future<Output = Result<ListFilesResult, String>> + Send {
        async move {
            list_files(args)
                .await
                .map_err(|e| format!("List files error: {e}"))
        }
    }

    fn bash(&self, args: BashInput) -> impl Future<Output = Result<BashResult, String>> + Send {
        async move {
            execute_command(args)
                .await
                .map_err(|e| format!("Bash command error: {e}"))
        }
    }

    fn read_background_bash(
        &self,
        handle: BackgroundProcessHandle,
        filter: Option<String>,
    ) -> impl Future<Output = Result<(ReadBackgroundBashOutput, Option<BackgroundProcessHandle>), String>> + Send {
        async move {
            read_background_bash(handle, filter)
                .await
                .map_err(|e| format!("Failed to get output: {e}"))
        }
    }
}
