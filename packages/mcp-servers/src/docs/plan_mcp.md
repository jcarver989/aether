MCP server for plan review workflows.

Exposes a single tool, `submit_plan`, that accepts an absolute markdown file path and returns `{ approved, feedback }`. The planner instructions themselves live in a user-customizable skill (e.g. `.aether/skills/plan/SKILL.md`) -- this server is just the review trigger.

Reviews are collected via MCP elicitation: the client receives a form elicitation with `ui: "planReview"` metadata (plus the plan path and markdown body) and returns an approve/deny decision with optional feedback.

# Construction

```rust,ignore
use mcp_servers::PlanMcp;

let server = PlanMcp::new();
```

# Tool provided

- **`submit_plan`** -- Reads a markdown file and returns a structured approval decision.
