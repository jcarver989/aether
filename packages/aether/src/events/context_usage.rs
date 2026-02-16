use agent_client_protocol::ExtNotification;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Custom notification method for context usage updates.
/// Per ACP extensibility spec, custom notifications must start with underscore.
pub const CONTEXT_USAGE_METHOD: &str = "_aether/context_usage";

/// Parameters for context usage update notifications.
///
/// This type is used for serialization/deserialization of `_aether/context_usage`
/// custom notification payload on both agent (server) and client sides.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ContextUsageParams {
    pub usage_ratio: f64,
    pub tokens_used: u32,
    pub context_limit: u32,
}

impl From<ContextUsageParams> for ExtNotification {
    fn from(params: ContextUsageParams) -> Self {
        let raw_value =
            serde_json::value::to_raw_value(&params).expect("ContextUsageParams is serializable");
        ExtNotification::new(CONTEXT_USAGE_METHOD, Arc::from(raw_value))
    }
}

#[cfg(test)]
mod tests {
    use super::{CONTEXT_USAGE_METHOD, ContextUsageParams};
    use agent_client_protocol::ExtNotification;

    #[test]
    fn test_context_usage_params_roundtrip() {
        let params = ContextUsageParams {
            usage_ratio: 0.75,
            tokens_used: 75000,
            context_limit: 100000,
        };

        let notification: ExtNotification = params.clone().into();

        assert_eq!(notification.method.as_ref(), CONTEXT_USAGE_METHOD);

        let parsed: ContextUsageParams =
            serde_json::from_str(notification.params.get()).expect("valid JSON");
        assert_eq!(parsed, params);
    }

    #[test]
    fn test_context_usage_method_has_underscore_prefix() {
        // ACP extensibility spec requires custom notifications start with underscore.
        assert!(
            CONTEXT_USAGE_METHOD.starts_with('_'),
            "Custom notification methods must start with underscore per ACP spec"
        );
    }
}
