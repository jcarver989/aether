# PlanMcp

MCP server that exposes a `submit_plan` tool that allows your agent to submit plans for user approval/feedback. Calling this tool triggers a MCP elicitation request to the client.

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
