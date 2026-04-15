# PlanMcp

Plan review server for implementation workflows.

Use `submit_plan` with an absolute `*-plan.md` path. The server either:

1. runs a configured external reviewer command (`--command <value>`), or
2. falls back to native MCP elicitation when no command is configured.

## Table of Contents

<!-- START doctoc generated TOC please keep comment here to allow auto update -->
<!-- DON'T EDIT THIS SECTION, INSTEAD RE-RUN doctoc TO UPDATE -->

- [Tools](#tools)
- [submit_plan](#submit_plan)
  - [Input](#input)
  - [Output](#output)
- [External reviewer command](#external-reviewer-command)

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

## External reviewer command

Pass `--command <value>` to execute an external reviewer with `bash -lc`.

The command receives a JSON payload on stdin:

```json
{
  "protocol": "aether-plan-review/v1",
  "cwd": "/workspace",
  "plan_path": "/workspace/example-plan.md",
  "permission_mode": "default",
  "tool_input": {
    "plan": "# Overview\n..."
  }
}
```

Accepted stdout response shape:

```json
{ "approved": true, "feedback": "optional" }
```

If your reviewer emits a different format (for example, harness-specific hook payloads), configure `--command` to invoke a wrapper/adapter that normalizes output into this shape.

