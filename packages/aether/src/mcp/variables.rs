use regex::Regex;
use std::env;

/// Expands environment variables in a string template.
///
/// Supports two formats:
/// - `$VAR` - Simple variable reference
/// - `${VAR}` - Bracketed variable reference
/// - `$$` - Escape sequence for literal `$`
///
/// # Examples
///
/// ```
/// # unsafe { std::env::set_var("TEST_VAR", "hello") };
/// # use aether::mcp::expand_env_vars;
/// let result = expand_env_vars("$TEST_VAR world").unwrap();
/// assert_eq!(result, "hello world");
///
/// let result = expand_env_vars("${TEST_VAR} world").unwrap();
/// assert_eq!(result, "hello world");
///
/// let result = expand_env_vars("$$VAR").unwrap();
/// assert_eq!(result, "$VAR");
/// # unsafe { std::env::remove_var("TEST_VAR") };
/// ```
pub fn expand_env_vars(template: &str) -> Result<String, VarError> {
    // Match: $$ (escape) | ${VAR} (bracketed) | $VAR (simple)
    let re = Regex::new(r"\$\$|\$\{([^}]+)\}|\$([a-zA-Z_][a-zA-Z0-9_]*)").unwrap();

    let mut result = String::with_capacity(template.len());
    let mut last_match = 0;

    for caps in re.captures_iter(template) {
        let m = caps.get(0).unwrap();
        result.push_str(&template[last_match..m.start()]);

        match (caps.get(1), caps.get(2)) {
            // ${VAR} - bracketed variable
            (Some(var), _) => {
                let var_name = var.as_str();
                let value = env::var(var_name)
                    .map_err(|_| VarError::NotFound(var_name.to_string()))?;
                result.push_str(&value);
            }
            // $VAR - simple variable
            (_, Some(var)) => {
                let var_name = var.as_str();
                let value = env::var(var_name)
                    .map_err(|_| VarError::NotFound(var_name.to_string()))?;
                result.push_str(&value);
            }
            // $$ - escape sequence
            _ => result.push('$'),
        }

        last_match = m.end();
    }

    result.push_str(&template[last_match..]);
    Ok(result)
}

#[derive(Debug)]
pub enum VarError {
    NotFound(String),
}

impl std::fmt::Display for VarError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VarError::NotFound(name) => write!(f, "Environment variable '{}' not found", name),
        }
    }
}

impl std::error::Error for VarError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_var() {
        unsafe { env::set_var("TEST_VAR_SIMPLE", "hello") };
        let result = expand_env_vars("$TEST_VAR_SIMPLE world").unwrap();
        assert_eq!(result, "hello world");
        unsafe { env::remove_var("TEST_VAR_SIMPLE") };
    }

    #[test]
    fn test_bracketed_var() {
        unsafe { env::set_var("TEST_VAR_BRACKET", "hello") };
        let result = expand_env_vars("${TEST_VAR_BRACKET} world").unwrap();
        assert_eq!(result, "hello world");
        unsafe { env::remove_var("TEST_VAR_BRACKET") };
    }

    #[test]
    fn test_escape_sequence() {
        let result = expand_env_vars("$$VAR").unwrap();
        assert_eq!(result, "$VAR");
    }

    #[test]
    fn test_multiple_vars() {
        unsafe {
            env::set_var("VAR1", "hello");
            env::set_var("VAR2", "world");
        }
        let result = expand_env_vars("$VAR1 ${VAR2}!").unwrap();
        assert_eq!(result, "hello world!");
        unsafe {
            env::remove_var("VAR1");
            env::remove_var("VAR2");
        }
    }

    #[test]
    fn test_missing_var() {
        let result = expand_env_vars("$MISSING_VAR");
        assert!(result.is_err());
        match result {
            Err(VarError::NotFound(name)) => assert_eq!(name, "MISSING_VAR"),
            _ => panic!("Expected NotFound error"),
        }
    }

    #[test]
    fn test_unclosed_brace_left_as_is() {
        // Unclosed braces are left as literal text since regex won't match
        let result = expand_env_vars("${VAR").unwrap();
        assert_eq!(result, "${VAR");
    }

    #[test]
    fn test_empty_string() {
        let result = expand_env_vars("").unwrap();
        assert_eq!(result, "");
    }

    #[test]
    fn test_no_vars() {
        let result = expand_env_vars("plain text").unwrap();
        assert_eq!(result, "plain text");
    }

    #[test]
    fn test_dollar_at_end() {
        let result = expand_env_vars("text$").unwrap();
        assert_eq!(result, "text$");
    }

    #[test]
    fn test_var_with_underscore() {
        unsafe { env::set_var("MY_TEST_VAR", "value") };
        let result = expand_env_vars("$MY_TEST_VAR").unwrap();
        assert_eq!(result, "value");
        unsafe { env::remove_var("MY_TEST_VAR") };
    }

    #[test]
    fn test_var_with_numbers() {
        unsafe { env::set_var("VAR123", "value") };
        let result = expand_env_vars("$VAR123").unwrap();
        assert_eq!(result, "value");
        unsafe { env::remove_var("VAR123") };
    }

    #[test]
    fn test_special_char_stops_var_name() {
        unsafe { env::set_var("VAR", "value") };
        let result = expand_env_vars("$VAR-suffix").unwrap();
        assert_eq!(result, "value-suffix");
        unsafe { env::remove_var("VAR") };
    }
}
