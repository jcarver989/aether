use serde::Deserialize;
use std::collections::HashMap;

/// Top-level: HashMap<provider_id, ProviderData>
pub type ModelsDevData = HashMap<String, ProviderData>;

#[derive(Debug, Deserialize)]
pub struct ProviderData {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub env: Vec<String>,
    #[serde(default)]
    pub models: HashMap<String, ModelData>,
}

#[derive(Debug, Deserialize)]
pub struct ModelData {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub tool_call: Option<bool>,
    #[serde(default)]
    pub reasoning: Option<bool>,
    #[serde(default)]
    pub cost: Option<CostData>,
    #[serde(default)]
    pub limit: Option<LimitData>,
}

#[derive(Debug, Deserialize)]
pub struct CostData {
    #[serde(default)]
    pub input: f64,
    #[serde(default)]
    pub output: f64,
}

#[derive(Debug, Deserialize)]
pub struct LimitData {
    #[serde(default)]
    pub context: u32,
    #[serde(default)]
    pub output: u32,
}
