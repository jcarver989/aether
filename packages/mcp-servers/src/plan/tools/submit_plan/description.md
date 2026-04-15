Submit a markdown plan file for review.

This tool accepts an absolute path to a plan markdown file, validates and reads it, then:

- runs an external reviewer command if `--command` is configured, or
- falls back to native MCP elicitation for approve/deny review.

Returns a normalized `{ approved, feedback }` result.
