Get compiler diagnostics (errors, warnings) from the language server.

Returns all diagnostics across the workspace, or filter to a specific file.
Useful for checking if your code changes introduced any errors before committing.

Example usage:
- Get all diagnostics: {}
- Get diagnostics for a file: {"file_path": "src/main.rs"}
