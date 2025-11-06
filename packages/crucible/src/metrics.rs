use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Evaluation metric returned by the LLM judge
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EvalMetric {
    Binary {
        success: bool,
        reason: String,
    },
    Numeric {
        score: f64,
        reason: String,
        max_score: f64,
    },
}

impl EvalMetric {
    /// Generate the JSON schema for this type
    pub fn json_schema() -> String {
        let schema = schemars::schema_for!(EvalMetric);
        serde_json::to_string_pretty(&schema).unwrap()
    }
}
