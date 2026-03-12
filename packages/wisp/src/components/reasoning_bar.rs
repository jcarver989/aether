use utils::ReasoningEffort;

/// Renders a compact 3-slot bracketed reasoning effort bar.
///
/// Visual mapping:
/// - `None` => `[···]` (all empty)
/// - `Low` => `[■··]` (1 filled)
/// - `Medium` => `[■■·]` (2 filled)
/// - `High` => `[■■■]` (3 filled)
pub(crate) fn reasoning_bar(effort: Option<ReasoningEffort>) -> String {
    const TOTAL: usize = 3;
    let filled = match effort {
        None => 0,
        Some(ReasoningEffort::Low) => 1,
        Some(ReasoningEffort::Medium) => 2,
        Some(ReasoningEffort::High) => 3,
    };
    let slots: Vec<&str> = (0..TOTAL)
        .map(|i| if i < filled { "■" } else { "·" })
        .collect();
    format!("[{}]", slots.join(""))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bar_none() {
        assert_eq!(reasoning_bar(None), "[···]");
    }

    #[test]
    fn bar_low() {
        assert_eq!(reasoning_bar(Some(ReasoningEffort::Low)), "[■··]");
    }

    #[test]
    fn bar_medium() {
        assert_eq!(reasoning_bar(Some(ReasoningEffort::Medium)), "[■■·]");
    }

    #[test]
    fn bar_high() {
        assert_eq!(reasoning_bar(Some(ReasoningEffort::High)), "[■■■]");
    }
}
