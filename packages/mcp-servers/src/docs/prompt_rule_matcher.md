Matches file reads against glob-triggered skill rules and injects them as system reminders.

When a file is read through [`CodingMcp`](crate::CodingMcp), the matcher checks if any skills in the project's `.aether/skills/` directory have `triggers.read` globs that match the file path. Matched rules are returned as additional context to include in the response.

# How it works

1. Skills declare read triggers as glob patterns in their frontmatter (e.g. `triggers.read: ["*.rs", "src/**/*.ts"]`).
2. On each `read_file` call, [`get_matched_rules`](PromptRuleMatcher::get_matched_rules) tests the file path against all trigger globs.
3. Matched rules fire **once per session** -- subsequent reads of files matching the same rule return an empty result. Call [`clear`](PromptRuleMatcher::clear) to reset.

# See also

- [`CodingMcp`](crate::CodingMcp) -- Integrates this matcher in its `read_file` tool.
