use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    Assistant,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CacheType {
    Ephemeral,
}


#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct AnthropicRequest {
    pub model: String,
    pub messages: Vec<AnthropicMessage>,
    pub max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<AnthropicSystemContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<AnthropicTool>>,
    pub stream: bool,
}

impl AnthropicRequest {
    pub fn new(model: String, messages: Vec<AnthropicMessage>) -> Self {
        Self {
            model,
            messages,
            max_tokens: 4096,
            temperature: None,
            system: None,
            tools: None,
            stream: false,
        }
    }

    pub fn with_max_tokens(mut self, max_tokens: u32) -> Self {
        self.max_tokens = max_tokens;
        self
    }

    pub fn with_temperature(mut self, temperature: f32) -> Self {
        self.temperature = Some(temperature);
        self
    }

    pub fn with_system(mut self, system: String) -> Self {
        self.system = Some(AnthropicSystemContent::Text(system));
        self
    }

    pub fn with_system_cached(mut self, system: String) -> Self {
        self.system = Some(AnthropicSystemContent::Blocks(vec![
            SystemContentBlock::Text {
                text: system,
                cache_control: Some(CacheControl::ephemeral()),
            }
        ]));
        self
    }

    pub fn with_tools(mut self, tools: Vec<AnthropicTool>) -> Self {
        self.tools = Some(tools);
        self
    }

    pub fn with_stream(mut self, stream: bool) -> Self {
        self.stream = stream;
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct AnthropicMessage {
    pub role: Role,
    pub content: AnthropicContent,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_control: Option<CacheControl>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum AnthropicContent {
    Text(String),
    Blocks(Vec<ContentBlock>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum AnthropicSystemContent {
    Text(String),
    Blocks(Vec<SystemContentBlock>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum SystemContentBlock {
    #[serde(rename = "text")]
    Text {
        text: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        cache_control: Option<CacheControl>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ContentBlock {
    #[serde(rename = "text")]
    Text {
        text: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        cache_control: Option<CacheControl>,
    },
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    #[serde(rename = "tool_result")]
    ToolResult {
        tool_use_id: String,
        content: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        is_error: Option<bool>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheControl {
    #[serde(rename = "type")]
    pub cache_type: CacheType,
}

impl CacheControl {
    pub fn ephemeral() -> Self {
        Self {
            cache_type: CacheType::Ephemeral,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct AnthropicTool {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_control: Option<CacheControl>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AnthropicStreamEvent {
    MessageStart {
        #[serde(flatten)]
        data: MessageStart,
    },
    ContentBlockStart {
        #[serde(flatten)]
        data: ContentBlockStart,
    },
    ContentBlockDelta {
        #[serde(flatten)]
        data: ContentBlockDelta,
    },
    ContentBlockStop {
        #[serde(flatten)]
        data: ContentBlockStop,
    },
    MessageDelta {
        #[serde(flatten)]
        data: MessageDelta,
    },
    MessageStop {
        #[serde(flatten)]
        data: MessageStop,
    },
    #[serde(rename = "error")]
    Error {
        #[serde(flatten)]
        data: ErrorEvent,
    },
    #[serde(rename = "ping")]
    Ping,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MessageStart {
    pub message: AnthropicResponseMessage,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct AnthropicResponseMessage {
    pub id: String,
    #[serde(rename = "type")]
    pub message_type: String,
    pub role: Role,
    pub content: Vec<serde_json::Value>,
    pub model: String,
    pub stop_reason: Option<String>,
    pub stop_sequence: Option<String>,
    pub usage: AnthropicUsage,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct AnthropicUsage {
    pub input_tokens: u32,
    pub output_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_creation_input_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_read_input_tokens: Option<u32>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ContentBlockStart {
    pub index: u32,
    pub content_block: ContentBlockStartData,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
pub enum ContentBlockStartData {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "tool_use")]
    ToolUse { id: String, name: String },
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ContentBlockDelta {
    pub index: u32,
    pub delta: ContentBlockDeltaData,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlockDeltaData {
    TextDelta { text: String },
    InputJsonDelta { partial_json: String },
}

#[derive(Debug, Clone, Deserialize)]
pub struct ContentBlockStop {
    pub index: u32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct MessageDelta {
    pub delta: MessageDeltaData,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct MessageDeltaData {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_sequence: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<AnthropicUsage>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MessageStop {}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ErrorEvent {
    pub error: AnthropicError,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct AnthropicError {
    #[serde(rename = "type")]
    pub error_type: String,
    pub message: String,
}

