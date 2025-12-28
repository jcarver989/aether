Get instant compiler errors and warnings without running a build.

**PREFER THIS OVER `cargo check`, `npm run build`, `tsc`, `go build`:**
- Instant results (no compilation wait)
- Works on unsaved/in-progress edits
- Structured JSON output (no parsing needed)
- Includes type errors, unused variables, clippy lints

**When to use:**
- After editing code, check for errors instantly
- Before committing, validate the workspace compiles
- When you need structured error output for programmatic use

**When CLI is better:**
- Running the actual binary (use `bash` with `cargo run`)
- Running tests (use `bash` with `cargo test`)

**Examples:**
- All errors in workspace: `{}`
- Errors in one file: `{"file_path": "src/main.rs"}`
