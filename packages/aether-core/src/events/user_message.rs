use llm::{ReasoningEffort, StreamingModelProvider, ToolDefinition};

/// Message from the user to the agent.
pub enum UserMessage {
    Text { content: String },
    Cancel,
    ClearContext,
    SwitchModel(Box<dyn StreamingModelProvider>),
    UpdateTools(Vec<ToolDefinition>),
    SetReasoningEffort(Option<ReasoningEffort>),
}

impl std::fmt::Debug for UserMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UserMessage::Text { content } => {
                f.debug_struct("Text").field("content", content).finish()
            }
            UserMessage::Cancel => write!(f, "Cancel"),
            UserMessage::ClearContext => write!(f, "ClearContext"),
            UserMessage::SwitchModel(provider) => f
                .debug_tuple("SwitchModel")
                .field(&provider.display_name())
                .finish(),
            UserMessage::UpdateTools(tools) => {
                f.debug_tuple("UpdateTools").field(&tools.len()).finish()
            }
            UserMessage::SetReasoningEffort(effort) => {
                f.debug_tuple("SetReasoningEffort").field(effort).finish()
            }
        }
    }
}

impl UserMessage {
    pub fn text(content: &str) -> Self {
        UserMessage::Text {
            content: content.to_string(),
        }
    }
}

impl From<&str> for UserMessage {
    fn from(value: &str) -> Self {
        UserMessage::Text {
            content: value.to_string(),
        }
    }
}
