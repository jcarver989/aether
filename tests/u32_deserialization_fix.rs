use serde::{Deserialize, Serialize};
use serde_json::json;

/// Test to reproduce the u32 deserialization error with -1 values
/// This simulates the cross-platform issue where some APIs return -1 for fields expecting u32

#[derive(Debug, Serialize, Deserialize)]
struct MockApiResponse {
    // These fields commonly appear in OpenAI-compatible APIs and might be u32
    #[serde(default)]
    prompt_tokens: Option<u32>,
    #[serde(default)]
    completion_tokens: Option<u32>, 
    #[serde(default)]
    total_tokens: Option<u32>,
    #[serde(default)]
    index: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize)]
struct FlexibleApiResponse {
    // More flexible fields that can handle -1 values
    #[serde(default, deserialize_with = "deserialize_flexible_u32")]
    prompt_tokens: Option<u32>,
    #[serde(default, deserialize_with = "deserialize_flexible_u32")]
    completion_tokens: Option<u32>,
    #[serde(default, deserialize_with = "deserialize_flexible_u32")]
    total_tokens: Option<u32>,
    #[serde(default, deserialize_with = "deserialize_flexible_u32")]
    index: Option<u32>,
}

fn deserialize_flexible_u32<'de, D>(deserializer: D) -> Result<Option<u32>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::Deserialize;
    
    let value = serde_json::Value::deserialize(deserializer)?;
    
    match value {
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                if i < 0 {
                    // Convert negative values to None instead of failing
                    Ok(None)
                } else {
                    Ok(Some(i as u32))
                }
            } else {
                Ok(None)
            }
        }
        serde_json::Value::Null => Ok(None),
        _ => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_u32_deserialization_error_reproduction() {
        // This JSON represents what might come from the API causing the error
        let problematic_json = json!({
            "prompt_tokens": -1,
            "completion_tokens": 100,
            "total_tokens": -1,
            "index": 0
        });

        // This should fail with the original struct
        let result: Result<MockApiResponse, _> = serde_json::from_value(problematic_json.clone());
        assert!(result.is_err(), "Should fail to deserialize -1 into u32");
        
        let error_msg = format!("{}", result.unwrap_err());
        assert!(error_msg.contains("invalid value: integer `-1`, expected u32"), 
                "Error should mention the -1 u32 issue");
    }

    #[test]
    fn test_flexible_deserialization_handles_negative_values() {
        // This JSON represents what might come from the API causing the error
        let problematic_json = json!({
            "prompt_tokens": -1,
            "completion_tokens": 100,
            "total_tokens": -1,
            "index": 0
        });

        // This should succeed with the flexible struct
        let result: Result<FlexibleApiResponse, _> = serde_json::from_value(problematic_json);
        assert!(result.is_ok(), "Should successfully deserialize with flexible handling");
        
        let response = result.unwrap();
        assert_eq!(response.prompt_tokens, None, "Negative values should become None");
        assert_eq!(response.completion_tokens, Some(100), "Positive values should be preserved");
        assert_eq!(response.total_tokens, None, "Negative values should become None");
        assert_eq!(response.index, Some(0), "Zero should be preserved");
    }

    #[test]
    fn test_flexible_deserialization_handles_normal_values() {
        let normal_json = json!({
            "prompt_tokens": 50,
            "completion_tokens": 100,
            "total_tokens": 150,
            "index": 0
        });

        let result: Result<FlexibleApiResponse, _> = serde_json::from_value(normal_json);
        assert!(result.is_ok(), "Should handle normal positive values");
        
        let response = result.unwrap();
        assert_eq!(response.prompt_tokens, Some(50));
        assert_eq!(response.completion_tokens, Some(100));
        assert_eq!(response.total_tokens, Some(150));
        assert_eq!(response.index, Some(0));
    }

    #[test]
    fn test_flexible_deserialization_handles_null_values() {
        let null_json = json!({
            "prompt_tokens": null,
            "completion_tokens": null,
            "total_tokens": null,
            "index": null
        });

        let result: Result<FlexibleApiResponse, _> = serde_json::from_value(null_json);
        assert!(result.is_ok(), "Should handle null values");
        
        let response = result.unwrap();
        assert_eq!(response.prompt_tokens, None);
        assert_eq!(response.completion_tokens, None);
        assert_eq!(response.total_tokens, None);
        assert_eq!(response.index, None);
    }
}