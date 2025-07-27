use serde_json::{Value, json};

/// Fix malformed JSON string arguments from LLM models.
/// Some models incorrectly return argument values as JSON strings instead of their actual types.
/// For example: {"query": "[\"value\"]"} instead of {"query": ["value"]}
fn fix_json_string_arguments(mut arguments: Value) -> Value {
    if let Some(obj) = arguments.as_object_mut() {
        for (_key, value) in obj.iter_mut() {
            if let Some(string_val) = value.as_str() {
                // Try to parse the string as JSON
                if let Ok(parsed_val) = serde_json::from_str::<Value>(string_val) {
                    // Only replace if the parsed value is not a string (to avoid infinite recursion)
                    match parsed_val {
                        Value::Array(_)
                        | Value::Object(_)
                        | Value::Number(_)
                        | Value::Bool(_)
                        | Value::Null => {
                            *value = parsed_val;
                        }
                        _ => {
                            // If it's still a string, don't replace
                        }
                    }
                }
            }
        }
    }
    arguments
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fix_malformed_array_argument() {
        let malformed = json!({
            "query": "[\"browser screenshots\"]"
        });
        let fixed = fix_json_string_arguments(malformed);

        assert_eq!(fixed["query"], json!(["browser screenshots"]));
    }

    #[test]
    fn test_fix_malformed_object_argument() {
        let malformed = json!({
            "config": "{\"option\": true, \"count\": 5}"
        });
        let fixed = fix_json_string_arguments(malformed);

        assert_eq!(fixed["config"], json!({"option": true, "count": 5}));
    }

    #[test]
    fn test_fix_malformed_number_argument() {
        let malformed = json!({
            "limit": "42"
        });
        let fixed = fix_json_string_arguments(malformed);

        assert_eq!(fixed["limit"], json!(42));
    }

    #[test]
    fn test_fix_malformed_boolean_argument() {
        let malformed = json!({
            "enabled": "true"
        });
        let fixed = fix_json_string_arguments(malformed);

        assert_eq!(fixed["enabled"], json!(true));
    }

    #[test]
    fn test_fix_malformed_null_argument() {
        let malformed = json!({
            "optional": "null"
        });
        let fixed = fix_json_string_arguments(malformed);

        assert_eq!(fixed["optional"], json!(null));
    }

    #[test]
    fn test_normal_arguments_unchanged() {
        let normal = json!({
            "query": ["browser screenshots"],
            "limit": 10,
            "enabled": true,
            "optional": null
        });
        let unchanged = fix_json_string_arguments(normal.clone());

        assert_eq!(unchanged, normal);
    }

    #[test]
    fn test_regular_string_unchanged() {
        let string_arg = json!({
            "message": "Hello world"
        });
        let unchanged = fix_json_string_arguments(string_arg.clone());

        assert_eq!(unchanged, string_arg);
    }

    #[test]
    fn test_mixed_arguments() {
        let mixed = json!({
            "query": "[\"browser screenshots\"]",  // Should be fixed
            "message": "Hello world",              // Should remain as string
            "limit": "42",                         // Should be fixed to number
            "existing_array": ["already", "array"], // Should remain unchanged
            "config": "{\"nested\": true}"         // Should be fixed to object
        });

        let fixed = fix_json_string_arguments(mixed);

        assert_eq!(fixed["query"], json!(["browser screenshots"]));
        assert_eq!(fixed["message"], json!("Hello world"));
        assert_eq!(fixed["limit"], json!(42));
        assert_eq!(fixed["existing_array"], json!(["already", "array"]));
        assert_eq!(fixed["config"], json!({"nested": true}));
    }

    #[test]
    fn test_empty_object() {
        let empty = json!({});
        let unchanged = fix_json_string_arguments(empty.clone());

        assert_eq!(unchanged, empty);
    }

    #[test]
    fn test_non_object_input() {
        let array_input = json!(["not", "an", "object"]);
        let unchanged = fix_json_string_arguments(array_input.clone());

        assert_eq!(unchanged, array_input);
    }

    #[test]
    fn test_invalid_json_string_unchanged() {
        let invalid_json = json!({
            "malformed": "{invalid json"
        });
        let unchanged = fix_json_string_arguments(invalid_json.clone());

        assert_eq!(unchanged, invalid_json);
    }
}
