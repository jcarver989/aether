use serde::{Deserialize, Serialize};

use crate::types::IsoString;

use super::{ToolCallError, ToolCallRequest, ToolCallResult};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ChatMessage {
    System {
        content: String,
        timestamp: IsoString,
    },
    User {
        content: String,
        timestamp: IsoString,
    },
    Assistant {
        content: String,
        timestamp: IsoString,
        tool_calls: Vec<ToolCallRequest>,
    },
    ToolCallResult(Result<ToolCallResult, ToolCallError>),
    Error {
        message: String,
        timestamp: IsoString,
    },
}
