//! CodeAssist API request and response types for Gemini OAuth authentication.
//!
//! When using OAuth tokens (instead of API keys), Gemini requires the CodeAssist
//! endpoint at `cloudcode-pa.googleapis.com` with its own request/response format.

use serde::{Deserialize, Serialize};

// ============================================================================
// Request Types
// ============================================================================

/// Top-level request for CodeAssist API
#[derive(Debug, Clone, Serialize)]
pub struct CAGenerateContentRequest {
    pub model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_prompt_id: Option<String>,
    pub request: VertexGenerateContentRequest,
}

/// Inner request matching Vertex AI format
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VertexGenerateContentRequest {
    pub contents: Vec<Content>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_instruction: Option<Content>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<Tool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_config: Option<ToolConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub generation_config: Option<GenerationConfig>,
}

/// Message content (user or model)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Content {
    pub role: String,
    pub parts: Vec<Part>,
}

/// Synthetic thought signature used to bypass validation for function calls
/// that don't have a real signature (e.g., when replaying history)
pub const SYNTHETIC_THOUGHT_SIGNATURE: &str = "skip_thought_signature_validator";

/// Part of a message content
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Part {
    Text {
        text: String,
    },
    FunctionCall {
        #[serde(rename = "functionCall")]
        function_call: FunctionCall,
        /// Thought signature required by Gemini for function calls in conversation history
        #[serde(rename = "thoughtSignature", skip_serializing_if = "Option::is_none")]
        thought_signature: Option<String>,
    },
    FunctionResponse {
        #[serde(rename = "functionResponse")]
        function_response: FunctionResponse,
    },
}

/// A function call from the model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionCall {
    pub name: String,
    pub args: serde_json::Value,
}

/// A function response to be sent back
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionResponse {
    pub name: String,
    pub response: serde_json::Value,
}

/// Tool definition container
#[derive(Debug, Clone, Serialize)]
pub struct Tool {
    #[serde(rename = "functionDeclarations")]
    pub function_declarations: Vec<FunctionDeclaration>,
}

/// Individual function declaration
#[derive(Debug, Clone, Serialize)]
pub struct FunctionDeclaration {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

/// Tool configuration
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub function_calling_config: Option<FunctionCallingConfig>,
}

/// Function calling configuration
#[derive(Debug, Clone, Serialize)]
pub struct FunctionCallingConfig {
    pub mode: String,
}

/// Generation configuration
#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct GenerationConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_k: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_sequences: Option<Vec<String>>,
}

// ============================================================================
// Response Types
// ============================================================================

/// Top-level response from CodeAssist API
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CaGenerateContentResponse {
    pub response: VertexGenerateContentResponse,
    #[serde(default)]
    #[allow(dead_code)]
    pub trace_id: Option<String>,
}

/// Inner response matching Vertex AI format
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VertexGenerateContentResponse {
    #[serde(default)]
    pub candidates: Vec<Candidate>,
    #[serde(default)]
    pub usage_metadata: Option<UsageMetadata>,
    #[serde(default)]
    #[allow(dead_code)]
    pub model_version: Option<String>,
}

/// A response candidate
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Candidate {
    #[serde(default)]
    #[allow(dead_code)]
    pub index: Option<u32>,
    #[serde(default)]
    pub content: Option<CandidateContent>,
    #[serde(default)]
    #[allow(dead_code)]
    pub finish_reason: Option<String>,
}

/// Content within a candidate
#[derive(Debug, Clone, Deserialize)]
pub struct CandidateContent {
    #[allow(dead_code)]
    pub role: String,
    pub parts: Vec<ResponsePart>,
}

/// Part of a response
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum ResponsePart {
    Text {
        text: String,
    },
    FunctionCall {
        #[serde(rename = "functionCall")]
        function_call: FunctionCallResponse,
    },
}

/// Function call in a response
#[derive(Debug, Clone, Deserialize)]
pub struct FunctionCallResponse {
    pub name: String,
    pub args: serde_json::Value,
}

/// Usage metadata
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageMetadata {
    #[serde(default)]
    pub prompt_token_count: Option<u32>,
    #[serde(default)]
    pub candidates_token_count: Option<u32>,
    #[serde(default)]
    #[allow(dead_code)]
    pub total_token_count: Option<u32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serialize_request() {
        let request = CAGenerateContentRequest {
            model: "gemini-2.0-flash".to_string(),
            project: None,
            user_prompt_id: Some("test-id".to_string()),
            request: VertexGenerateContentRequest {
                contents: vec![Content {
                    role: "user".to_string(),
                    parts: vec![Part::Text {
                        text: "Hello".to_string(),
                    }],
                }],
                system_instruction: None,
                tools: None,
                tool_config: None,
                generation_config: None,
            },
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("gemini-2.0-flash"));
        assert!(json.contains("user"));
        assert!(json.contains("Hello"));
    }

    #[test]
    fn test_deserialize_response() {
        let json = r#"{
            "response": {
                "candidates": [{
                    "content": {
                        "role": "model",
                        "parts": [{"text": "Hello!"}]
                    },
                    "finishReason": "STOP"
                }],
                "usageMetadata": {
                    "promptTokenCount": 10,
                    "candidatesTokenCount": 5
                }
            }
        }"#;

        let response: CaGenerateContentResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.response.candidates.len(), 1);

        let candidate = &response.response.candidates[0];
        let content = candidate.content.as_ref().unwrap();
        assert_eq!(content.role, "model");

        match &content.parts[0] {
            ResponsePart::Text { text } => assert_eq!(text, "Hello!"),
            _ => panic!("Expected text part"),
        }
    }

    #[test]
    fn test_deserialize_function_call_response() {
        let json = r#"{
            "response": {
                "candidates": [{
                    "content": {
                        "role": "model",
                        "parts": [{
                            "functionCall": {
                                "name": "get_weather",
                                "args": {"location": "NYC"}
                            }
                        }]
                    }
                }]
            }
        }"#;

        let response: CaGenerateContentResponse = serde_json::from_str(json).unwrap();
        let content = response.response.candidates[0].content.as_ref().unwrap();

        match &content.parts[0] {
            ResponsePart::FunctionCall { function_call } => {
                assert_eq!(function_call.name, "get_weather");
                assert_eq!(function_call.args["location"], "NYC");
            }
            _ => panic!("Expected function call"),
        }
    }
}
