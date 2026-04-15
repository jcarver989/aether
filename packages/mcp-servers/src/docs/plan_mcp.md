MCP server for plan review workflows.

Provides a single tool, `submit_plan`, that accepts an absolute markdown file path and returns `{ approved, feedback }`.

# Review modes

- **External command mode** (`--command <value>`) -- Executes the command with `bash -lc` in the workspace root, sends a JSON payload on stdin, and parses stdout into a plan review decision.
- **Native elicitation mode** (no command configured) -- Prompts the user directly via MCP elicitation to approve or deny the plan, optionally collecting feedback.

# Construction

```rust,ignore
use mcp_servers::{PlanMcp, PlanMcpArgs};

let server = PlanMcp::new();

let server = PlanMcp::from_args(vec!["--command".into(), "./scripts/plannotator-mcp-adapter".into()]).unwrap();

let args = PlanMcpArgs { command: Some("./scripts/plannotator-mcp-adapter".into()) };
let server = PlanMcp::from_args_with_root(args, "/my/project".into());
```

# Tool provided

- **`submit_plan`** -- Reads a markdown file and returns a structured approval decision.
