# CLAUDE.md - mcp-lexicon

## Writing Evals

Evals are integration tests that validate MCP tool behavior in realistic scenarios. They live in `tests/evals/` and follow a consistent structure.

### Eval Directory Structure

Each eval is a directory containing:

```
tests/evals/<eval_name>/
├── prompt.md          # User instruction that triggers the tool usage
├── eval.json          # Test assertions and optional git config
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

3. **Write `eval.json`** - Validation criteria using these assertion types:

   ```json
   {
     "assertions": [
       {
         "type": "LLMJudge",
         "data": {
           "prompt": "Did the agent successfully [describe expected behavior]?"
         }
       }
     ]
   }
   ```

   **Assertion Types:**

   - **LLMJudge**: Uses an LLM to evaluate if the agent succeeded
   - **FileExists**: Verifies a file or directory exists (`"data": { "path": "..." }`)
   - **FileMatches**: Checks if file contains specific content (`"data": { "path": "...", "content": "..." }`)
   - **CommandExitCode**: Runs a command and checks its exit code (`"data": { "command": "...", "expected_code": 0 }`)

4. **Create test files in `src/`** (if needed):
   - Add any files the eval needs to operate on
   - For example: files to search through, edit, or read

### Examples

#### Simple Tool Usage (bash tool)
```
tests/evals/simple_bash_command/
├── prompt.md          # "Run echo 'Hello from bash!'"
├── eval.json          # LLMJudge: "Did agent run echo and show output?"
└── src/main.rs       # Placeholder file
```

#### File Operations (edit tool)
```
tests/evals/edit_single_file/
├── prompt.md          # "Fix the typo in main.rs"
├── eval.json          # FileMatches: Check "World" not "Wolrd"
└── src/main.rs       # File containing the typo
```

#### Complex Workflows (git operations)
```
tests/evals/git_operations/
├── prompt.md          # "Init repo, create README, commit"
├── eval.json          # FileExists: .git, README.md + FileMatches + LLMJudge
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
