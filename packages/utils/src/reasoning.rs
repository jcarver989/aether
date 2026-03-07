use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReasoningEffort {
    Low,
    Medium,
    High,
}

impl ReasoningEffort {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
        }
    }

    pub fn all() -> &'static [ReasoningEffort] {
        &[Self::Low, Self::Medium, Self::High]
    }

    /// Parse a string into an optional effort level.
    /// Accepts "none" / "" as `None`, and "low"/"medium"/"high" as `Some`.
    pub fn parse(s: &str) -> Result<Option<Self>, String> {
        match s {
            "none" | "" => Ok(None),
            other => other.parse().map(Some),
        }
    }
}

impl fmt::Display for ReasoningEffort {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for ReasoningEffort {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "low" => Ok(Self::Low),
            "medium" => Ok(Self::Medium),
            "high" => Ok(Self::High),
            _ => Err(format!("Unknown reasoning effort: '{s}'")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_roundtrip() {
        for effort in ReasoningEffort::all() {
            let s = effort.to_string();
            let parsed: ReasoningEffort = s.parse().unwrap();
            assert_eq!(*effort, parsed);
        }
    }

    #[test]
    fn as_str_matches_display() {
        for effort in ReasoningEffort::all() {
            assert_eq!(effort.as_str(), effort.to_string());
        }
    }

    #[test]
    fn from_str_rejects_unknown() {
        assert!("max".parse::<ReasoningEffort>().is_err());
    }

    #[test]
    fn all_returns_three_variants() {
        assert_eq!(ReasoningEffort::all().len(), 3);
    }

    #[test]
    fn parse_none_and_empty() {
        assert_eq!(ReasoningEffort::parse("none").unwrap(), None);
        assert_eq!(ReasoningEffort::parse("").unwrap(), None);
    }

    #[test]
    fn parse_valid_levels() {
        assert_eq!(
            ReasoningEffort::parse("high").unwrap(),
            Some(ReasoningEffort::High)
        );
        assert_eq!(
            ReasoningEffort::parse("low").unwrap(),
            Some(ReasoningEffort::Low)
        );
    }

    #[test]
    fn parse_rejects_unknown() {
        assert!(ReasoningEffort::parse("max").is_err());
    }

    #[test]
    fn serialize_produces_lowercase() {
        for effort in ReasoningEffort::all() {
            let json = serde_json::to_value(effort).unwrap();
            assert_eq!(json.as_str().unwrap(), effort.as_str());
        }
    }
}
