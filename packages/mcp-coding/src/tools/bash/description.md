Executes a given bash command in a persistent shell session with optional timeout, ensuring proper handling and security measures.

IMPORTANT: This tool is for terminal operations like git, npm, docker, cargo, etc. DO NOT use it for file operations (reading, writing, editing, searching, finding files) - use the specialized tools for this instead.

Usage:
- The command argument is required
- You can specify an optional timeout in milliseconds (up to 600000ms / 10 minutes). If not specified, commands will timeout after 120000ms (2 minutes)
- It is very helpful if you write a clear, concise description of what this command does in 5-10 words in the description parameter
- If the output exceeds 30000 characters, output will be truncated before being returned to you
- You can use the `run_in_background` parameter to run the command in the background, which allows you to continue working while the command runs. You can monitor the output using the `read_background_bash` tool as it becomes available. Never use `run_in_background` to run 'sleep' as it will return immediately. You do not need to use '&' at the end of the command when using this parameter
- Avoid using bash with the `find`, `grep`, `cat`, `head`, `tail`, `sed`, `awk`, or `echo` commands, unless explicitly instructed or when these commands are truly necessary for the task. Instead, always prefer using the dedicated tools for these commands:
  - File search: Use find tool (NOT find or ls bash commands)
  - Content search: Use grep tool (NOT grep or rg bash commands)
  - Read files: Use `read_file` tool (NOT cat/head/tail)
  - Edit files: Use `edit_file` tool (NOT sed/awk)
  - Write files: Use `write_file` tool (NOT echo >/cat <<EOF)
- When issuing multiple commands:
  - If the commands are independent and can run in parallel, make multiple bash tool calls in a single message. For example, if you need to run "git status" and "git diff", send a single message with two bash tool calls in parallel
  - If the commands depend on each other and must run sequentially, use a single bash call with '&&' to chain them together (e.g., `git add . && git commit -m "message" && git push`). For instance, if one operation must complete before another starts (like mkdir before cp, `write_file` before bash for git operations, or git add before git commit), run these operations sequentially instead
  - Use ';' only when you need to run commands sequentially but don't care if earlier commands fail
  - DO NOT use newlines to separate commands (newlines are ok in quoted strings)
