Find all references to a symbol across the codebase.

Find everywhere a symbol is used. Useful for understanding impact before refactoring.
Requires reading the file first to know the line number.

Example: To find all usages of a function `spawn` that appears on line 15,
use {"file_path": "/path/to/file.rs", "symbol": "spawn", "line": "15"}
