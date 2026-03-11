Gets instant compiler errors and warnings without running a build.

**Prefer this over `cargo check`, `npm run build`, `tsc`, `go build`.**

## Usage

The tool has two explicit modes:

**Workspace-wide diagnostics:**

```json
{"input":{"scope":"workspace"}}
```

**Single-file diagnostics:**

```json
{"input":{"scope":"file","filePath":"/absolute/path/to/file.rs"}}
```

## Parameters

- `input` — **required**, wrapper object for the diagnostics query
- `input.scope` — **required**, either `"workspace"` or `"file"`
- `input.filePath` — required when `input.scope="file"`, must be an absolute path to an existing file

## Response

The response includes the resolved scope so you can verify what was executed:

- `scope` — the scope that was queried
- `workspaceRoot` — present when scope is `"workspace"`
- `filePath` — present when scope is `"file"`
- `diagnostics` — list of errors, warnings, etc.
- `summary` — counts by severity

## Why Use This

- Instant results (no compilation wait)
- Works on unsaved/in-progress edits
- Structured output (no parsing needed)

## When CLI is Better

- Running the actual binary → `bash` with `cargo run`
- Running tests → `bash` with `cargo test`

## Important

- Directory paths are **invalid** and will return an error
- Invalid requests return explicit validation errors, never empty success results
