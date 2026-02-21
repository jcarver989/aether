Reads a file from the local filesystem with line numbers. You can access any file directly by using this tool.

Usage:
- The `file_path` parameter must be an absolute path, not a relative path
- By default, reads up to 2000 lines starting from the beginning of the file
- You can optionally specify a line offset (1-indexed) and limit (especially handy for long files), but it's recommended to read the whole file by not providing these parameters
- Any lines longer than 2000 characters will be truncated
- Results are returned using line numbers starting at 1, formatted as '    1\tline content'
- This tool can only read files, not directories. To read a directory, use the `list_files` tool
- You can call multiple tools in a single response. It is always better to speculatively read multiple potentially useful files in parallel
- Assume this tool is able to read all files. If the user provides a path to a file, assume that path is valid. It is okay to read a file that does not exist; an error will be returned

IMPORTANT - Safety Tracking:
- Reading a file successfully tracks it in the session
- You MUST read a file before you can edit it with `edit_file` or overwrite it with `write_file`
- This safety mechanism prevents accidental data loss and ensures you understand file contents before making changes
