---
description: Generate a succinct PR description based on code changes
argument-hint: "[base-branch]"
allowed-tools:
  - Bash(git branch:*)
  - Bash(git merge-base:*)
  - Bash(git rev-parse:*)
  - Bash(git diff:*)
  - Bash(git log:*)
  - Bash(gh pr view:*)
  - Bash(gh pr create:*)
  - Bash(gh pr edit:*)
  - Read
  - Grep
  - Glob
  - AskUserQuestion
---

You are an experienced engineer creating a clear, succinct pull request description. Your goal is to help reviewers quickly understand what changed and why.

## Step 1: Determine the Base Branch

**Check for user-provided base branch**: If the user provided an argument (`$1`), use that as the base branch. Otherwise, auto-detect:

If `$1` is provided, use it directly as the base. Otherwise, determine the base by:

1. First, check for an upstream tracking branch: `git rev-parse --abbrev-ref --symbolic-full-name @{u} 2>/dev/null`
   - If this succeeds and returns a branch name, extract the branch name (remove `origin/` prefix) and use that as the base
   - This handles cases where the current branch was created from an epic branch or any other feature branch
2. If no tracking branch exists, fall back to `main` as the base: `git merge-base HEAD main 2>/dev/null`
3. Get current branch for reference: `git branch --show-current`

## Step 2: Gather Context

Collect information about the changes:

```bash
# Get changed files
git diff --name-status <base>...HEAD

# Get commit messages for context
git log <base>...HEAD --oneline

# Get the actual diff
git diff <base>...HEAD
```

## Step 3: Check for PR Template

Look for a PR template in the repository:

- `.github/PULL_REQUEST_TEMPLATE.md`
- `.github/pull_request_template.md`
- `PULL_REQUEST_TEMPLATE.md`
- `docs/PULL_REQUEST_TEMPLATE.md`

If found, read it to understand the expected format and sections.

## Step 4: Analyze Changes

Review the changes to understand:

- **What** changed: Which files, features, or components
- **Why** it changed: The purpose or problem being solved
- **How** it changed: Key implementation decisions or approaches
- **Impact**: What reviewers should pay attention to

Consider reading relevant files for additional context if needed.

## Step 5: Generate Description

Create a **succinct** description that:

- Starts with a clear summary (1-2 sentences)
- Follows the PR template format if one exists
- Highlights the most important information for reviewers
- Uses bullet points for readability
- Avoids unnecessary verbosity or obvious details
- Focuses on the "why" and "what", not exhaustive "how"

**Be concise**: Reviewers will see the diff. Your job is to provide context and highlight what matters.

## Step 6: Present to User and Get Action

Show the generated PR description to the user, then ask what they want to do next.

**Preferred approach**: Use the `AskUserQuestion` tool if available with this structure:

```json
{
  "questions": [
    {
      "header": "Next action",
      "question": "What would you like to do with this PR description?",
      "multiSelect": false,
      "options": [
        {
          "label": "Make changes",
          "description": "Revise the description based on your feedback"
        },
        {
          "label": "Create/update PR",
          "description": "Create a new PR or update the existing one with this description"
        }
      ]
    }
  ]
}
```

Based on the user's response:

- **If "Make changes"**: Ask for feedback and regenerate the description
- **If "Create/update PR"**: Use the `gh` CLI to:
  - Check if a PR exists for this branch: `gh pr view --json number,title`
  - If no PR exists: Create a draft PR with `gh pr create --draft --title "..." --body "..."`
  - If PR exists: Update the description with `gh pr edit <number> --body "..."`
- **If "Other"**: The user can provide custom instructions via text input

**Fallback**: If the `AskUserQuestion` tool is not available, present the options in text and wait for the user's response before taking action.

**Important**: Never execute `gh` commands without user confirmation.

### Handling Markdown in PR Descriptions

When creating or updating PRs with `gh pr create` or `gh pr edit`, use a heredoc **without quotes** to preserve markdown formatting:

```bash
gh pr create --draft --title "..." --body "$(cat <<EOF
Your PR description here with backticks ` and code blocks:
\`\`\`
code here
\`\`\`
EOF
)"
```

**Critical**: Use `<<EOF` (not `<<'EOF'`) so that backticks and other markdown syntax are preserved correctly. Only escape dollar signs (`\$`) if they appear in the description text to prevent shell variable expansion.

---

## Example Workflow

Here's how a complete execution should look:

1. **Determine base branch** → `main`
2. **Gather context** → Read diff, commits, and PR template
3. **Generate description** → Create succinct PR description following template
4. **Show to user** → Display the generated description
5. **Ask for action** → Call `AskUserQuestion` tool:

   ```text
   AskUserQuestion tool call with:
   - Question: "What would you like to do with this PR description?"
   - Options: "Make changes" or "Create/update PR"
   ```

6. **Execute based on choice**:
   - If "Make changes" → Get feedback and regenerate
   - If "Create/update PR" → Execute `gh pr create` or `gh pr edit`

**Note**: Keep it brief and focused. The goal is to help reviewers understand the change quickly, not to document every line of code.
