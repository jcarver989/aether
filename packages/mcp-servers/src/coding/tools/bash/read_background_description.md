Retrieves output from a running or completed background bash shell.

Usage:
- Takes a `bash_id` parameter identifying the shell (returned from bash tool when `run_in_background` is true)
- Always returns only new output since the last check
- Returns stdout and stderr output along with shell status (running/completed/failed)
- Supports optional regex filtering to show only lines matching a pattern
- Use this tool when you need to monitor or check the output of a long-running shell
- When a shell is completed, the output is final and the shell ID becomes invalid
