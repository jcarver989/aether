use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Evaluation metric returned by a LLM judge
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EvalMetric {
    Binary(BinaryMetric),
    Numeric(NumericMetric),
}

impl EvalMetric {
    /// Generate the JSON schema for this type
    pub fn json_schema() -> String {
        let schema = schemars::schema_for!(EvalMetric);
        serde_json::to_string_pretty(&schema).unwrap()
    }
}

/// Binary success/failure metric
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct BinaryMetric {
    pub success: bool,
    pub reason: String,
}

impl BinaryMetric {
    /// Generate the JSON schema for this type
    pub fn json_schema() -> String {
        let schema = schemars::schema_for!(BinaryMetric);
        serde_json::to_string_pretty(&schema).unwrap()
    }
}

/// Numeric score metric
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct NumericMetric {
    pub score: f64,
    pub reason: String,
    pub max_score: f64,
}

impl NumericMetric {
    /// Generate the JSON schema for this type
    pub fn json_schema() -> String {
        let schema = schemars::schema_for!(NumericMetric);
        serde_json::to_string_pretty(&schema).unwrap()
    }
}
