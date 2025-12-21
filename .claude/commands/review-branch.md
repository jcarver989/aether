---
description: Perform a comprehensive senior-level code review of changes on the current branch
argument-hint: '[base-branch]'
allowed-tools:
  - Bash(git branch:*)
  - Bash(git merge-base:*)
  - Bash(git rev-parse:*)
  - Bash(git diff:*)
  - Bash(git log:*)
  - Bash(sed:*)
  - Read
  - Grep
  - Glob
---

You are a senior staff engineer conducting a code review. Provide terse, scannable feedback.

## Step 1: Determine Base Branch

If user provided an argument (`$1`), use it. Otherwise:

1. Try upstream: `git rev-parse --abbrev-ref --symbolic-full-name @{u} 2>/dev/null | sed 's|^origin/||'`
2. Fall back to main: `git merge-base HEAD main`

## Step 2: Review Changes

```bash
git diff --name-status <base>...HEAD
git diff <base>...HEAD
```

Focus on: bugs, security issues, unclear code, missing tests, violations of project patterns (check CLAUDE.md).

## Step 3: Output Format

**CRITICAL**: Be concise and scannable. Each section should take no more than 10 lines of vertical space.

### 🚨 Critical

Bugs, security issues, breaks functionality.

### 💡 Consider

Quality improvements, suggestions.

### ✅ Good Stuff

Note 1-3 things done well.

**IMPORTANT**:

- Each finding should take up no more than about 10 vertical lines to keep things scannable.
- If no major issues, be brief - just note good patterns
- Skip minor style issues unless pervasive
- Be direct and specific
