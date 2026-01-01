# Task Schema Update Plan: Structured Completion Results

## Context

### Current State
The task system is being refactored from a simple `todo_write` tool to a sophisticated `tasks` system with hierarchical task trees, dependencies, and persistence. The current `Task` struct uses a simple `result: Option<String>` field for completion output.

### Problem
The Factory.ai article ["Evaluating Compression for AI Agents"](https://factory.ai/news/evaluating-compression) found that:

1. **Compression ratio is the wrong metric** — what matters is "tokens per task" (retaining information needed to avoid re-fetching and re-exploring dead ends)

2. **Structured summarization wins** — Factory's approach maintains "explicit sections for different information types: session intent, file modifications, decisions made, and next steps"

3. **Four probe types matter** for evaluating retention quality:
   - **Recall**: Factual retention ("What was the original error?")
   - **Artifact**: File tracking ("Which files were modified?")
   - **Continuation**: Task planning capability
   - **Decision**: Reasoning chain preservation

4. **Anchored iterative summarization** prevents "silent information drift" across compression cycles

The current `result: Option<String>` is unstructured, meaning:
- Agents can omit critical information in free-form prose
- Decisions get buried, reasoning chains lost
- Re-compression causes drift

### Key Insight: Git Handles File Artifacts

The article identified artifact tracking as the weakest point (2.19–2.45/5.0). However, in a git repository, the working tree already tracks file modifications:

```bash
git diff --name-only        # modified files
git status --porcelain      # created/deleted files
git diff path/to/file       # what changed
```

**The schema should not duplicate what git already knows.** We only need to track what git cannot tell us.

---

## Proposed Schema Changes

### 1. New `TaskResult` Struct

Replace `result: Option<String>` with a structured type:

```rust
/// Structured task completion result aligned with compression-resilient probe types
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct TaskResult {
    /// Executive summary (1-3 sentences)
    pub summary: String,
    
    /// Key decisions made during task execution
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub decisions: Vec<Decision>,
    
    /// Critical facts discovered (error messages, config values, patterns)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub findings: Vec<Finding>,
    
    /// Continuation context for downstream tasks
    #[serde(default, skip_serializing_if = "Continuation::is_empty")]
    pub continuation: Continuation,
    
    /// Files read but not modified (git doesn't track reads)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub files_read: Vec<String>,
    
    /// External resources accessed (git doesn't track these)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub resources: Vec<ResourceRef>,
}
```

### 2. Decision Tracking (Reasoning Chain Preservation)

```rust
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct Decision {
    /// What was decided
    pub what: String,
    
    /// Why this choice was made
    pub why: String,
    
    /// Alternatives considered but rejected
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rejected: Vec<String>,
}
```

### 3. Findings (Factual Retention)

```rust
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct Finding {
    /// Category: "error", "config", "pattern", "constraint", etc.
    pub kind: String,
    
    /// The actual finding
    pub content: String,
    
    /// Where this was found (file path, command output, etc.)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}
```

### 4. Continuation Context (Task Planning Capability)

```rust
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct Continuation {
    /// Suggested next steps identified during execution
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub next_steps: Vec<String>,
    
    /// Blockers or dependencies that couldn't be resolved
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub blockers: Vec<String>,
    
    /// Open questions needing human input or further research
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub open_questions: Vec<String>,
}

impl Continuation {
    pub fn is_empty(&self) -> bool {
        self.next_steps.is_empty() 
            && self.blockers.is_empty() 
            && self.open_questions.is_empty()
    }
}
```

### 5. External Resources

```rust
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct ResourceRef {
    /// URI (URL, API endpoint, database connection, etc.)
    pub uri: String,
    
    /// What was retrieved or learned from this resource
    pub summary: String,
}
```

---

## What We Track vs What Git Tracks

| Information | Source | In Schema? |
|-------------|--------|------------|
| Files modified | `git diff --name-only` | No |
| Files created | `git status` | No |
| File change content | `git diff <path>` | No |
| Files read (not modified) | Not in git | **Yes** |
| External resources | Not in git | **Yes** |
| Decisions made | Not in git | **Yes** |
| Facts discovered | Not in git | **Yes** |
| Next steps / blockers | Not in git | **Yes** |

---

## Updated `TaskCompleteInput`

```rust
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct TaskCompleteInput {
    /// Task ID to mark as completed
    pub id: String,

    /// Structured result (preferred)
    #[serde(default)]
    pub result: Option<TaskResult>,
    
    /// Simple text result (for backwards compatibility / simple tasks)
    #[serde(default)]
    pub result_text: Option<String>,
}
```

---

## Updated Tool Description (`description_complete.md`)

```markdown
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
```

---

## Implementation Plan

### Phase 1: Add Schema Types to `types.rs`
- Add `TaskResult`, `Decision`, `Finding`, `Continuation`, `ResourceRef` structs
- Add `impl is_empty()` helpers for conditional serialization

### Phase 2: Update `Task` Struct
- Change `result: Option<String>` to `result: Option<TaskResult>`
- Add `result_text: Option<String>` for simple/legacy completions

### Phase 3: Update `TaskCompleteInput`
- Accept either `result: TaskResult` or `result_text: String`
- Update `execute_task_complete` to handle both

### Phase 4: Update Tool Description
- Rewrite `description_complete.md` with structured guidance
- Add examples showing proper decision/finding capture
- Note that file modifications come from git, not the schema

### Phase 5: Tests
- Add tests for structured result serialization
- Add tests for backwards-compatible `result_text` path
- Verify `is_empty()` helpers work correctly for skip_serializing

---

## Evaluation Criteria

Following Factory's probe-based approach, the schema should enable answering:

1. **Recall probe**: "What was the original error?" → Check `findings` where `kind == "error"`
2. **Artifact probe**: "Which files were modified?" → Query `git diff --name-only`
3. **Continuation probe**: "What should happen next?" → Check `continuation.next_steps`
4. **Decision probe**: "Why was X approach chosen?" → Check `decisions[].why`

---

## Future Considerations

- **Auto-population**: The task system could observe tool calls (Read, WebFetch) and auto-populate `files_read` and `resources` rather than relying on agent self-reporting
- **Validation**: Warn if `result` has empty `decisions` for complex tasks
- **Compression testing**: Run Factory-style probes against stored results to measure retention quality
