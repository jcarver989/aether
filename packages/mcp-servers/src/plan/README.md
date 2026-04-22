# PlanMcp

MCP server that exposes a `submit_plan` tool that allows your agent to submit plans for user approval/feedback. By default, calling this tool triggers an MCP elicitation request to the client (e.g. Wisp renders a plan-review TUI panel).

Alternatively, the server can hand the plan off to an arbitrary external CLI. Any trailing positional tokens in the `mcp.json` `args` array are treated as the submit command; the plan's absolute path is appended as its final argument, and its stdout is forwarded verbatim to the agent as `feedback`.

```json
{
  "servers": {
    "plan": {
      "type": "in-memory",
      "args": ["contextbridge", "plan", "--project", "foo"]
    }
  }
}
```

With the above config, calling `submit_plan` with `planPath=/tmp/plan.md` invokes:

```
contextbridge plan --project foo /tmp/plan.md
```

A non-zero exit code surfaces as a tool error; an exit code of `0` returns `{ "approved": false, "feedback": "<stdout>" }` regardless of stdout content — the agent reads the feedback and decides how to proceed.

## Table of Contents

<!-- START doctoc generated TOC please keep comment here to allow auto update -->
<!-- DON'T EDIT THIS SECTION, INSTEAD RE-RUN doctoc TO UPDATE -->

- [Tools](#tools)
- [submit_plan](#submit_plan)
  - [Input](#input)
  - [Output](#output)

<!-- END doctoc generated TOC please keep comment here to allow auto update -->

## Tools

| Tool | Description |
|------|-------------|
| `submit_plan` | Submit a markdown plan file for review and approval. |

## submit_plan

### Input

| Field | Type | Description |
|-------|------|-------------|
| `planPath` | string | Absolute path to a markdown plan file. |

### Output

| Field | Type | Description |
|-------|------|-------------|
| `approved` | bool | Whether the plan is approved. |
| `feedback` | string \| null | Optional reviewer feedback, typically present on denial. |
