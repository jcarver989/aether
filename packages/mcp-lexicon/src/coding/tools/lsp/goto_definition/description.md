Go to the definition of a symbol.

Find where a symbol (function, type, variable, etc.) is defined.
Requires reading the file first to know the line number.

Example: After reading a file that has `let client = LspClient::new()` on line 42,
use {"file_path": "/path/to/file.rs", "symbol": "LspClient", "line": "42"}
to find where LspClient is defined.
