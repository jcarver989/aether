# CLAUDE.md - mcp-lexicon

## Writing Evals

Evals are integration tests that validate MCP tool behavior in realistic scenarios. They are defined programmatically in `src/evals.rs` using the Crucible API.

### Eval Directory Structure

Each eval has a directory containing:

```
tests/evals/<eval_name>/
├── prompt.md          # User instruction loaded at runtime via Prompt::file()
└── src/              # Optional: test files for the eval to operate on
    └── *.rs
```

### Creating a New Eval

1. **Create the directory structure:**
   ```bash
   mkdir -p tests/evals/<eval_name>/src
   ```

2. **Write `prompt.md`** - A clear, natural user request:
   ```markdown
   # [Task description]

   [Clear instruction that would naturally use the tool being tested]
   ```

3. **Add the eval to `src/evals.rs`** - Define it programmatically:

   ```rust
   Eval::new(
       "eval_name",
       load_prompt("eval_name")?,  // Loads from tests/evals/eval_name/prompt.md
       WorkingDirectory::local(tests_dir.join("evals/eval_name/src"))?,  // or empty()
       vec![
           EvalAssertion::file_exists("file.txt"),
           EvalAssertion::file_matches("file.txt", "content"),
           EvalAssertion::llm_judge("Did the agent succeed?"),
           EvalAssertion::command_succeeds("cargo test"),
           EvalAssertion::tool_call_at_least("bash", 1),
       ],
   ),
   ```

### Assertion Types

Available assertion builders:

- **File assertions:**
  - `EvalAssertion::file_exists(path)` - Verifies file/directory exists
  - `EvalAssertion::file_matches(path, content)` - Checks file contains substring

- **Command assertions:**
  - `EvalAssertion::command_exit_code(cmd, code)` - Runs command, checks exit code
  - `EvalAssertion::command_succeeds(cmd)` - Shorthand for exit code 0

- **Tool call assertions:**
  - `EvalAssertion::tool_call(name)` - Checks tool was called
  - `EvalAssertion::tool_call_with_args(name, json)` - Checks tool + arguments
  - `EvalAssertion::tool_call_exact(name, count)` - Checks exact call count
  - `EvalAssertion::tool_call_at_least(name, count)` - Checks minimum calls
  - `EvalAssertion::tool_call_at_most(name, count)` - Checks maximum calls

- **LLM Judge:**
  - `EvalAssertion::llm_judge(prompt)` - Uses LLM to evaluate success

### Working Directory Options

- `WorkingDirectory::empty()` - Fresh empty temp directory
- `WorkingDirectory::local(path)` - Copies files from path into temp directory
- `WorkingDirectory::git_repo(url, start_sha, gold_sha, subdir)` - Clones git repo

### Examples

#### Simple Tool Usage (bash tool)
```rust
Eval::new(
    "simple_bash_command",
    load_prompt("simple_bash_command")?,
    WorkingDirectory::empty()?,
    vec![
        EvalAssertion::llm_judge("Did agent run echo and show output?"),
    ],
),
```

#### File Operations (edit tool)
```rust
Eval::new(
    "edit_single_file",
    load_prompt("edit_single_file")?,
    WorkingDirectory::local(tests_dir.join("evals/edit_single_file/src"))?,
    vec![
        EvalAssertion::file_matches("src/main.rs", "Hello, World!"),
    ],
),
```

#### Complex Workflows (git operations)
```rust
Eval::new(
    "git_operations",
    load_prompt("git_operations")?,
    WorkingDirectory::empty()?,
    vec![
        EvalAssertion::file_exists(".git"),
        EvalAssertion::file_exists("README.md"),
        EvalAssertion::file_matches("README.md", "# My Project"),
        EvalAssertion::llm_judge("Did agent init repo, create README, and commit?"),
    ],
),
```

### Best Practices

- **Edit prompts without recompiling**: Prompts are loaded from markdown files at runtime
- **Use multiple assertion types**: Combine LLMJudge with file/command checks
- **Keep evals focused**: Test one primary tool behavior per eval
- **Name descriptively**: Use `<tool>_<scenario>` naming (e.g., `bash_command_chaining`)
- **Type-safe assertions**: Compile-time validation of eval structure
