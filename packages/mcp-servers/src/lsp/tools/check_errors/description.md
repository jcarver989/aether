Gets instant compiler errors and warnings without running a build.

**Prefer this over `cargo check`, `npm run build`, `tsc`, `go build`.**

## Usage

```json
{}
{"file_path": "src/main.rs"}
```

- `file_path` — optional, filter to specific file

**Returns:** type errors, unused variables, clippy lints, etc. Structured JSON output.

## Why Use This

- Instant results (no compilation wait)
- Works on unsaved/in-progress edits
- Structured output (no parsing needed)

## When CLI is Better

- Running the actual binary → `bash` with `cargo run`
- Running tests → `bash` with `cargo test`
