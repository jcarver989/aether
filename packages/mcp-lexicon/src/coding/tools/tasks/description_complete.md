Mark a task as completed with structured findings.

Provide a **structured result** to ensure findings survive context compression:

| Field | Purpose | What to Include |
|-------|---------|-----------------|
| `summary` | 1-3 sentence headline | The key outcome |
| `decisions` | Reasoning chain | What was decided, why, what was rejected |
| `findings` | Facts discovered | Errors, configs, patterns, constraints |
| `continuation` | Next steps | Follow-ups, blockers, open questions |
| `files_read` | Files examined | Paths read but not modified |
| `resources` | External sources | URLs, APIs accessed with summaries |

Note: File modifications are tracked by git (`git diff --name-only`), so you
do not need to list them in the result.

When a task is completed:
- Its status changes to `completed`
- Any tasks that depended on it may become ready to start
- The result is stored for later reference

The response includes `newly_ready` - a list of tasks that became ready to start after this completion.

**Example - Structured completion:**
```json
{
  "id": "at-a1b2c3d4.1",
  "result": {
    "summary": "Identified 5 API endpoints using deprecated auth pattern",
    "decisions": [{
      "what": "Defer JWT migration until after v2.0",
      "why": "Breaking change requires client SDK updates",
      "rejected": ["Immediate migration", "Dual auth support"]
    }],
    "findings": [
      {
        "kind": "pattern",
        "content": "All endpoints use validate_session() for auth",
        "source": "src/api/*.rs"
      },
      {
        "kind": "error",
        "content": "Session tokens expire after 1 hour with no refresh",
        "source": "logs/auth.log"
      }
    ],
    "continuation": {
      "next_steps": ["Create migration guide", "Add deprecation warnings"],
      "blockers": ["Need product decision on migration timeline"],
      "open_questions": ["Should we support both auth methods during transition?"]
    },
    "files_read": ["src/api/auth.rs", "src/api/users.rs", "docs/AUTH.md"],
    "resources": [{
      "uri": "https://docs.rs/jsonwebtoken",
      "summary": "JWT library docs - supports RS256 and ES256"
    }]
  }
}
```

**Example - Simple completion (for trivial tasks):**
```json
{
  "id": "at-a1b2c3d4.2",
  "result_text": "Fixed typo in README"
}
```
