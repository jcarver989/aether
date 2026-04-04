# Crucible

A Rust library for writing automated tests (evals) for LLM-powered agents.

## Table of Contents

<!-- START doctoc generated TOC please keep comment here to allow auto update -->
<!-- DON'T EDIT THIS SECTION, INSTEAD RE-RUN doctoc TO UPDATE -->

- [Quick Start](#quick-start)
- [Directory Structure](#directory-structure)
- [Eval Configuration](#eval-configuration)
  - [Basic Eval (with local files)](#basic-eval-with-local-files)
  - [Git Repository Eval](#git-repository-eval)
- [Assertion Types](#assertion-types)
  - [ToolCall Options](#toolcall-options)
- [Configuration](#configuration)
- [Output](#output)
- [Development](#development)

<!-- END doctoc generated TOC please keep comment here to allow auto update -->

## Quick Start

```rust
use crucible::{AetherRunner, Eval, EvalAssertion, EvalRunner, EvalsConfig, FileSystemStore, WorkingDirectory};
use llm::providers::openrouter::OpenRouterProvider;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let llm = OpenRouterProvider::default("anthropic/claude-sonnet-4-5-20250929")?;
    let judge_llm = OpenRouterProvider::default("anthropic/claude-sonnet-4-5-20250929")?;

    let runner = AetherRunner::new(llm);
    let store = FileSystemStore::new("./results".into())?;
    let config = EvalsConfig::new(judge_llm)
        .with_batch_size(3)
        .with_serve(true);

    let evals = vec![
        Eval::new(
            "hello_world",
            "Write 'Hello, World!' to hello.txt",
            WorkingDirectory::empty()?,
            vec![EvalAssertion::file_exists("hello.txt")],
        ),
    ];

    let run_id = EvalRunner::new(runner, store)
        .run_evals(evals, config)
        .await?;

    println!("Run: {run_id}");
    Ok(())
}
```

## Directory Structure

```
my-agent/
├── AGENTS.md          # Agent system prompt (optional)
├── mcp.json          # MCP servers config (optional)
└── evals/
    └── test-name/
        ├── prompt.md      # Task for the agent
        ├── eval.json      # Assertions
        └── src/           # Working directory files (optional)
```

## Eval Configuration

### Basic Eval (with local files)

**evals/test-name/prompt.md:**
```markdown
Read data.txt and count lines containing "error". Write result to output.txt.
```

**evals/test-name/eval.json:**
```json
{
  "assertions": [
    {
      "type": "ToolCall",
      "data": { "name": "read_file", "count": { "Exact": 1 } }
    },
    {
      "type": "FileExists",
      "data": { "path": "output.txt" }
    },
    {
      "type": "LLMJudge",
      "data": { "prompt": "Did the agent correctly count error lines?" }
    }
  ]
}
```

### Git Repository Eval

Use real codebases instead of `src/` directory:

```json
{
  "git": {
    "url": "https://github.com/user/repo",
    "start_commit": "abc123",
    "eval_commit": "def456",
    "subdir": "packages/api"  // optional
  },
  "assertions": [
    {
      "type": "CommandExitCode",
      "data": { "command": "npm test", "expected_code": 0 }
    }
  ]
}
```

The agent starts at `start_commit` and `eval_commit` provides reference for LLM judge assertions.

## Assertion Types

| Type | Description | Example |
|------|-------------|---------|
| `ToolCall` | Validate tool usage | `{ "name": "read_file", "count": { "AtLeast": 1 } }` |
| `FileExists` | Check file exists | `{ "path": "output.txt" }` |
| `FileMatches` | Check file content | `{ "path": "out.txt", "content": "expected" }` |
| `LLMJudge` | LLM evaluates result | `{ "prompt": "Did agent solve the task?" }` |
| `CommandExitCode` | Check command exit code | `{ "command": "cargo test", "expected_code": 0 }` |

### ToolCall Options

```json
{
  "type": "ToolCall",
  "data": {
    "name": "write_file",
    "arguments": { "path": "out.txt" },  // Optional: exact match
    "count": { "Exact": 1 }  // Or "AtLeast", "AtMost"
  }
}
```

## Configuration

```rust
use std::time::Duration;

// Control batching and rate limiting
let config = EvalsConfig::new(judge_llm)
    .with_batch_size(5)
    .with_batch_delay(Duration::from_secs(1))
    .with_serve(true);

// Custom output directory and MCP server factories
let eval_runner = EvalRunner::new(runner, store)
    .with_output_dir("./results".into());
```

## Output

```
crucible_output_20241104_120000/
├── summary.json      # Overall results
├── traces.jsonl     # Full execution traces
├── results/         # Individual eval results
└── report/          # HTML report (served at localhost:3000)
```

## Development

```bash
cargo test -p crucible
cargo build -p crucible
cargo clippy -p crucible
```
