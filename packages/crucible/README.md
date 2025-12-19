# Crucible

A Rust library for writing automated tests (evals) for LLM-powered agents.

## Quick Start

```rust
use crucible::{Crucible, EvalsConfig};
use aether::llm::openrouter::OpenRouter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let llm = OpenRouter::new("anthropic/claude-sonnet-4-5-20250929")?;
    let judge_llm = OpenRouter::new("anthropic/claude-sonnet-4-5-20250929")?;

    let config = EvalsConfig::new(llm, judge_llm)
        .with_batch_size(3)
        .with_serve(true);  // View results at localhost:3000

    let summary = Crucible::new("./my-agent".into())
        .run_evals(config)
        .await?;

    println!("Passed: {}/{}", summary.passed, summary.total);
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
// Control batching and rate limiting
let config = EvalsConfig::new(llm, judge_llm)
    .with_batch_size(5)
    .with_batch_delay(Duration::from_secs(1))
    .with_serve(true);

// Custom output directory and MCP servers
let crucible = Crucible::new("./my-agent".into())
    .with_output_dir("./results".into())
    .with_server_factory("my-server", factory);
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
