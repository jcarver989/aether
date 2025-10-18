# CLAUDE.md - mcp-lexicon

## Writing Evals

Evals are integration tests that validate MCP tool behavior in realistic scenarios. They live in `tests/evals/` and follow a consistent structure.

### Eval Directory Structure

Each eval is a directory containing:

```
tests/evals/<eval_name>/
├── prompt.md          # User instruction that triggers the tool usage
├── assertions.json    # Test assertions to validate success
└── src/              # Optional: test files for the eval to operate on
    └── *.rs
```

### Creating a New Eval

1. **Create the directory structure:**
   ```bash
   mkdir -p tests/evals/<eval_name>/src
   ```

2. **Write `prompt.md`** - A clear, natural user request that should trigger the tool:
   ```markdown
   # [Task description]

   [Clear instruction that would naturally use the tool being tested]
   ```

3. **Write `assertions.json`** - Validation criteria using these assertion types:

   - **LLMJudge**: Uses an LLM to evaluate if the agent succeeded
     ```json
     {
       "type": "LLMJudge",
       "data": {
         "prompt": "Did the agent successfully [describe expected behavior]?"
       }
     }
     ```

   - **FileExists**: Verifies a file or directory exists
     ```json
     {
       "type": "FileExists",
       "data": {
         "path": "relative/path/to/file"
       }
     }
     ```

   - **FileMatches**: Checks if file contains specific content
     ```json
     {
       "type": "FileMatches",
       "data": {
         "path": "relative/path/to/file",
         "content": "expected content substring"
       }
     }
     ```

   - **CommandExitCode**: Runs a command and checks its exit code
     ```json
     {
       "type": "CommandExitCode",
       "data": {
         "command": "cargo check",
         "expected_code": 0
       }
     }
     ```

4. **Create test files in `src/`** (if needed):
   - Add any files the eval needs to operate on
   - For example: files to search through, edit, or read

### Examples

#### Simple Tool Usage (bash tool)
```
tests/evals/simple_bash_command/
├── prompt.md          # "Run echo 'Hello from bash!'"
├── assertions.json    # LLMJudge: "Did agent run echo and show output?"
└── src/main.rs       # Placeholder file
```

#### File Operations (edit tool)
```
tests/evals/edit_single_file/
├── prompt.md          # "Fix the typo in main.rs"
├── assertions.json    # FileMatches: Check "World" not "Wolrd"
└── src/main.rs       # File containing the typo
```

#### Complex Workflows (git operations)
```
tests/evals/git_operations/
├── prompt.md          # "Init repo, create README, commit"
├── assertions.json    # FileExists: .git, README.md + FileMatches + LLMJudge
└── src/main.rs       # Placeholder
```

### Best Practices

- **Prompt should be natural**: Write how a user would actually ask
- **Use multiple assertion types**: Combine LLMJudge with FileExists/FileMatches for robust validation
- **Keep evals focused**: Test one primary tool behavior per eval
- **Include edge cases**: Create evals for error conditions, complex inputs, etc.
- **Name descriptively**: Use `<tool>_<scenario>` naming (e.g., `bash_command_chaining`)

### Tool-Specific Eval Ideas

- **Bash**: Command execution, chaining, environment variables, git workflows
- **Read**: File reading, directory reading (should fail), line ranges
- **Write**: Creating new files, overwriting, path validation
- **Edit**: Single replacements, multiple edits, non-existent strings
- **Grep**: Pattern matching, regex, glob filtering, output modes
- **TodoWrite**: Task creation, status updates, complex workflows
