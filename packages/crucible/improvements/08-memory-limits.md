# Memory Limits and Streaming

**Priority:** 🟢 P2 - Medium
**Impact:** Low-Medium
**Effort:** Low
**Estimated LOC:** ~50

## Problem

The eval runner accumulates ALL agent messages in memory in `packages/crucible/src/evals/runner.rs`:

```rust
// Line ~150
let mut messages = Vec::new();

while let Some(msg) = rx.recv().await {
    match &msg {
        AgentRunnerMessage::Done => break,
        _ => {
            messages.push(msg.clone());  // Unbounded growth!
        }
    }
}
```

### Impact

For long-running agents with many tool calls:

**Example:**
- Agent makes 500 tool calls
- Each tool call generates 3 messages (call, result, text)
- Average message size: 2KB
- Total memory: 500 × 3 × 2KB = **3MB per eval**

For 100 concurrent evals: **300MB** just for messages!

### Worst Case

```rust
// Agent in infinite loop calling tools
loop {
    read_file("data.json");  // Returns 100KB each time
}

// Memory usage grows unbounded until OOM
```

## Solution

Stream messages directly to storage and keep only recent messages in memory for LLM judge context.

### Configuration

```rust
// In packages/crucible/src/evals/config.rs

pub struct EvalsConfig<J> {
    // ... existing fields ...

    /// Maximum messages to keep in memory (None = unlimited)
    pub max_messages_in_memory: Option<usize>,
}

impl<J> EvalsConfig<J> {
    pub fn with_max_messages_in_memory(mut self, max: usize) -> Self {
        self.max_messages_in_memory = Some(max);
        self
    }
}
```

### Bounded Message Buffer

```rust
// In packages/crucible/src/evals/runner.rs

use std::collections::VecDeque;

/// Fixed-size circular buffer for messages
struct MessageBuffer {
    messages: VecDeque<AgentRunnerMessage>,
    max_size: Option<usize>,
    total_received: usize,
}

impl MessageBuffer {
    fn new(max_size: Option<usize>) -> Self {
        Self {
            messages: VecDeque::new(),
            max_size,
            total_received: 0,
        }
    }

    fn push(&mut self, msg: AgentRunnerMessage) {
        self.total_received += 1;

        if let Some(max) = self.max_size {
            if self.messages.len() >= max {
                // Remove oldest message
                self.messages.pop_front();
                tracing::debug!(
                    "Message buffer full, dropped oldest message (total: {})",
                    self.total_received
                );
            }
        }

        self.messages.push_back(msg);
    }

    fn as_slice(&self) -> Vec<AgentRunnerMessage> {
        self.messages.iter().cloned().collect()
    }

    fn total_received(&self) -> usize {
        self.total_received
    }
}
```

### Update Message Collection

```rust
// In packages/crucible/src/evals/runner.rs

async fn run_single_eval<R, T, J>(
    // ... params
    config: &EvalsConfig<J>,
) -> Result<EvalResult, Box<dyn std::error::Error>>
{
    // ... existing setup ...

    let mut message_buffer = MessageBuffer::new(config.max_messages_in_memory);

    // Stream messages
    while let Some(msg) = rx.recv().await {
        // Write to trace immediately (before buffering)
        tracing::info!("{:?}", msg);

        match &msg {
            AgentRunnerMessage::Done => break,
            _ => {
                message_buffer.push(msg.clone());
            }
        }
    }

    tracing::info!(
        "Agent completed. Received {} messages, kept {} in memory",
        message_buffer.total_received(),
        message_buffer.messages.len()
    );

    let messages = message_buffer.as_slice();

    // ... rest of assertions using messages ...
}
```

### LLM Judge Context Adjustment

Since we're only keeping recent messages, LLM judges need to handle partial context:

```rust
// In packages/crucible/src/evals/assertion.rs

impl LlmJudgeContext<'_> {
    /// Check if message history is truncated
    pub fn is_truncated(&self) -> bool {
        // This would need to be passed from MessageBuffer
        false // Placeholder
    }

    /// Get total messages received (including truncated)
    pub fn total_messages(&self) -> usize {
        self.messages.len()
    }
}
```

Update judge prompts to mention truncation:

```rust
let context_note = if message_buffer.was_truncated() {
    format!(
        "\nNote: Showing last {} of {} total messages.",
        messages.len(),
        message_buffer.total_received()
    )
} else {
    String::new()
};

let judge_prompt = format!(
    "Task: {}\n{}\nMessages: {:?}\n\nDid the agent succeed?",
    eval.task_prompt,
    context_note,
    messages
);
```

## Alternative: Stream Everything to Disk

For cases where all messages are needed (e.g., full transcript analysis):

```rust
/// Write messages to temporary file instead of memory
struct DiskMessageStore {
    file: tokio::fs::File,
    count: usize,
}

impl DiskMessageStore {
    async fn new() -> Result<Self> {
        let temp_file = tempfile::NamedTempFile::new()?;
        let file = tokio::fs::File::create(temp_file.path()).await?;

        Ok(Self { file, count: 0 })
    }

    async fn append(&mut self, msg: &AgentRunnerMessage) -> Result<()> {
        let json = serde_json::to_string(msg)?;
        self.file.write_all(json.as_bytes()).await?;
        self.file.write_all(b"\n").await?;
        self.count += 1;
        Ok(())
    }

    async fn read_all(&mut self) -> Result<Vec<AgentRunnerMessage>> {
        // Rewind and read all messages
        self.file.seek(std::io::SeekFrom::Start(0)).await?;
        let mut reader = tokio::io::BufReader::new(&self.file);
        let mut messages = Vec::new();

        let mut line = String::new();
        while reader.read_line(&mut line).await? > 0 {
            let msg: AgentRunnerMessage = serde_json::from_str(&line)?;
            messages.push(msg);
            line.clear();
        }

        Ok(messages)
    }
}
```

## Usage Examples

### Default (Unlimited)

```rust
let config = EvalsConfig::new(judge_llm);
// All messages kept in memory
```

### Memory-Constrained

```rust
let config = EvalsConfig::new(judge_llm)
    .with_max_messages_in_memory(1000);  // Keep last 1000 messages

// For 100 concurrent evals with 1000 messages each at 2KB avg:
// Memory: 100 * 1000 * 2KB = 200MB (bounded!)
```

### Aggressive Limits

```rust
let config = EvalsConfig::new(judge_llm)
    .with_max_messages_in_memory(100);  // Only last 100 messages

// Useful for CI/CD where memory is constrained
```

## Files to Change

1. `packages/crucible/src/evals/config.rs` - Add `max_messages_in_memory` field
2. `packages/crucible/src/evals/runner.rs` - Add `MessageBuffer` struct
3. `packages/crucible/src/evals/runner.rs` - Update message collection loop
4. `packages/crucible/src/evals/assertion.rs` - Add truncation awareness to `LlmJudgeContext`

## Benefits

1. **Bounded Memory**: Prevent OOM on long-running agents
2. **Predictable Resource Usage**: Know max memory per eval
3. **CI/CD Friendly**: Run on memory-constrained environments
4. **Graceful Degradation**: Still get recent context for judges

## Trade-offs

### What We Lose

1. **Full Message History**: LLM judges only see recent messages
2. **Tool Call Analysis**: If checking "agent never called X", might miss early calls

### Mitigations

1. **Configurable Limit**: Users can set high limits or unlimited
2. **Full Traces Still Written**: All messages in JSONL file for post-analysis
3. **Tool Call Assertions Unaffected**: They scan all messages before truncation

## Testing Strategy

1. Create agent that generates 10,000 messages
2. Set limit to 100, verify only last 100 kept
3. Verify memory usage stays bounded
4. Verify assertions still work correctly
5. Test with limit=None (unlimited) works as before
