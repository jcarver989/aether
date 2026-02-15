use regex::Regex;
use std::env;

/// Expands environment variables in a string template.
///
/// Supports two formats:
/// - `$VAR` - Simple variable reference
/// - `${VAR}` - Bracketed variable reference
/// - `$$` - Escape sequence for literal `$`
pub fn expand_env_vars(template: &str) -> Result<String, VarError> {
    let escape_re = Regex::new(r"\$\$").unwrap();
    let bracketed_re = Regex::new(r"\$\{([^}]+)\}").unwrap();
    let simple_re = Regex::new(r"\$([a-zA-Z_][a-zA-Z0-9_]*)").unwrap();

    const ESCAPE_PLACEHOLDER: &str = "\x00ESCAPED_DOLLAR\x00";

    // Replace $$ with placeholder
    let result = escape_re.replace_all(template, ESCAPE_PLACEHOLDER);

    // Replace ${VAR} with env var, tracking any missing vars
    let mut missing_var = None;
    let result = bracketed_re.replace_all(&result, |caps: &regex::Captures| {
        let var_name = &caps[1];
        match env::var(var_name) {
            Ok(value) => value,
            Err(_) => {
                missing_var = Some(var_name.to_string());
                caps[0].to_string() // Keep original if not found
            }
        }
    });
    if let Some(var) = missing_var {
        return Err(VarError::NotFound(var));
    }

    // Replace $VAR with env var
    let result = simple_re.replace_all(&result, |caps: &regex::Captures| {
        let var_name = &caps[1];
        match env::var(var_name) {
            Ok(value) => value,
            Err(_) => {
                missing_var = Some(var_name.to_string());
                caps[0].to_string()
            }
        }
    });
    if let Some(var) = missing_var {
        return Err(VarError::NotFound(var));
    }

    // Replace placeholder with $
    let result = result.replace(ESCAPE_PLACEHOLDER, "$");

    Ok(result)
}

#[derive(Debug)]
pub enum VarError {
    NotFound(String),
}

impl std::fmt::Display for VarError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VarError::NotFound(name) => write!(f, "Environment variable '{name}' not found"),
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
