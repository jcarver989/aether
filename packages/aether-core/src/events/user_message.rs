use llm::{ContentBlock, ReasoningEffort, StreamingModelProvider, ToolDefinition};

/// Message from the user to the agent.
pub enum UserMessage {
    Text { content: Vec<ContentBlock> },
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
            content: vec![ContentBlock::text(content)],
        }
    }

    pub fn with_content(content: Vec<ContentBlock>) -> Self {
        UserMessage::Text { content }
    }
}

impl From<&str> for UserMessage {
    fn from(value: &str) -> Self {
        UserMessage::text(value)
    }
}
