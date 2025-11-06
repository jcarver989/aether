use serde::{Deserialize, Serialize};

/// A simplified DTO for storing trace events
///
/// This is mapped from tracing-serde-structured types which aren't suitable
/// for long-term storage (not Send/Sync with 'static lifetime).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum TraceEvent {
    Event {
        level: String,
        target: String,
        message: Option<String>,
        fields: serde_json::Value,
    },
    Span {
        name: String,
        level: String,
        target: String,
        fields: serde_json::Value,
    },
    Other,
}
