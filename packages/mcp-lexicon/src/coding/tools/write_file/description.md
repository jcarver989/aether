Writes a file to the local filesystem, replacing the entire file contents.

Usage:
- This tool will overwrite the existing file if there is one at the provided path
- ALWAYS prefer editing existing files in the codebase using edit_file. NEVER write new files unless explicitly required
- NEVER proactively create documentation files (*.md) or README files. Only create documentation files if explicitly requested by the user
- Only use emojis if the user explicitly requests it. Avoid writing emojis to files unless asked
- The file_path parameter must be an absolute path, not a relative path
- Creates parent directories automatically if they don't exist

IMPORTANT - Safety Requirements:
- If the file already exists, you MUST use read_file on it first before calling write_file
- This tool will return an error if you attempt to overwrite an existing file without reading it first
- New files (that don't exist yet) can be created without reading
- This safety check prevents accidental data loss by ensuring you see the current file contents before overwriting them
