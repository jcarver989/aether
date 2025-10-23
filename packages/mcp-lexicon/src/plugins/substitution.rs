use regex::Regex;

/// Substitute parameters in a prompt template
///
/// Supports:
/// - `$1`, `$2`, etc. for positional arguments
/// - `$ARGUMENTS` for all arguments as a single string
///
/// Arguments are provided as a JSON map where keys are stringified indices ("0", "1", etc.)
/// or as named parameters that can be accessed by position
pub fn substitute_parameters(
    template: &str,
    arguments: &Option<serde_json::Map<String, serde_json::Value>>,
) -> String {
    let mut result = template.to_string();

    if let Some(args) = arguments {
        let mut positional_args = Vec::new();

        // Try to extract numbered arguments (0, 1, 2, etc.)
        let mut i = 0;
        while let Some(value) = args.get(&i.to_string()) {
            positional_args.push(value_to_string(value));
            i += 1;
        }

        // If no numbered args, use all values in order (sorted by key)
        if positional_args.is_empty() {
            let mut sorted_args: Vec<_> = args.iter().collect();
            sorted_args.sort_by_key(|(k, _)| *k);
            positional_args = sorted_args
                .into_iter()
                .map(|(_, v)| value_to_string(v))
                .collect();
        }

        // Replace $ARGUMENTS with all arguments joined
        let all_args = positional_args.join(" ");
        result = result.replace("$ARGUMENTS", &all_args);

        // Replace positional parameters $1, $2, etc.
        // Use regex to avoid issues with $10 vs $1
        let positional_regex = Regex::new(r"\$(\d+)").unwrap();
        result = positional_regex
            .replace_all(&result, |caps: &regex::Captures| {
                let index: usize = caps[1].parse().unwrap();
                if index > 0 && index <= positional_args.len() {
                    positional_args[index - 1].clone()
                } else {
                    caps[0].to_string() // Keep original if out of bounds
                }
            })
            .to_string();
    } else {
        // No arguments provided, replace with empty strings
        result = result.replace("$ARGUMENTS", "");
        let positional_regex = Regex::new(r"\$\d+").unwrap();
        result = positional_regex.replace_all(&result, "").to_string();
    }

    result
}

/// Convert a JSON value to a string representation
fn value_to_string(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Null => String::new(),
        serde_json::Value::Array(_) | serde_json::Value::Object(_) => {
            serde_json::to_string(value).unwrap_or_default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_substitute_positional_args() {
        let template = "Review this $1 code in $2 language";
        let mut args = serde_json::Map::new();
        args.insert("0".to_string(), serde_json::json!("Rust"));
        args.insert("1".to_string(), serde_json::json!("Python"));

        let result = substitute_parameters(template, &Some(args));
        assert_eq!(result, "Review this Rust code in Python language");
    }

    #[test]
    fn test_substitute_all_arguments() {
        let template = "Analyze: $ARGUMENTS";
        let mut args = serde_json::Map::new();
        args.insert("0".to_string(), serde_json::json!("foo"));
        args.insert("1".to_string(), serde_json::json!("bar"));
        args.insert("2".to_string(), serde_json::json!("baz"));

        let result = substitute_parameters(template, &Some(args));
        assert_eq!(result, "Analyze: foo bar baz");
    }

    #[test]
    fn test_no_arguments() {
        let template = "Hello $1, welcome to $2. Args: $ARGUMENTS";
        let result = substitute_parameters(template, &None);
        assert_eq!(result, "Hello , welcome to . Args: ");
    }

    #[test]
    fn test_mixed_substitution() {
        let template = "First: $1, Second: $2, All: $ARGUMENTS";
        let mut args = serde_json::Map::new();
        args.insert("0".to_string(), serde_json::json!("alpha"));
        args.insert("1".to_string(), serde_json::json!("beta"));

        let result = substitute_parameters(template, &Some(args));
        assert_eq!(result, "First: alpha, Second: beta, All: alpha beta");
    }

    #[test]
    fn test_out_of_bounds() {
        let template = "Available: $1, Missing: $5";
        let mut args = serde_json::Map::new();
        args.insert("0".to_string(), serde_json::json!("exists"));

        let result = substitute_parameters(template, &Some(args));
        assert_eq!(result, "Available: exists, Missing: $5");
    }
}
