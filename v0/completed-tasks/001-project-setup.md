# Task 001: Project Setup and Dependencies

## Objective
Initialize the Rust project with all required dependencies and basic project structure.

## Requirements
1. Create a new Rust project using Cargo
2. Configure Cargo.toml with the following dependencies:
   - tokio = { version = "1", features = ["full"] }
   - async-openai = "0.x" (use latest version)
   - ratatui = "0.x" (use latest version)
   - serde = { version = "1", features = ["derive"] }
   - serde_json = "1"
   - clap = "4"
   - anyhow = "1"
   - crossterm = "0.x" (use latest version)

3. Create the basic directory structure:
   ```
   src/
   ├── main.rs
   ├── config/
   │   └── mod.rs
   ├── llm/
   │   └── mod.rs
   ├── mcp/
   │   └── mod.rs
   └── ui/
       └── mod.rs
   ```

## Deliverables
- Initialized Cargo project
- Cargo.toml with all dependencies
- Basic directory structure with module files
- Verify project compiles with `cargo check`

## Notes
- Use workspace-friendly structure if planning future extensions
- Ensure all module files have basic module declarations