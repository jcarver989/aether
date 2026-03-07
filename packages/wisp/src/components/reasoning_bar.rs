use utils::ReasoningEffort;

/// Renders a 3-slot reasoning effort bar.
///
/// Visual mapping:
/// - `None` => `▱▱▱` (all empty)
/// - `Low` => `▰▱▱` (1 filled)
/// - `Medium` => `▰▰▱` (2 filled)
/// - `High` => `▰▰▰` (3 filled)
pub(crate) fn reasoning_bar(effort: Option<ReasoningEffort>) -> String {
    const TOTAL: usize = 3;
    let filled = match effort {
        None => 0,
        Some(ReasoningEffort::Low) => 1,
        Some(ReasoningEffort::Medium) => 2,
        Some(ReasoningEffort::High) => 3,
    };
    let filled_part: String = "▰".repeat(filled.min(TOTAL));
    let empty_part: String = "▱".repeat(TOTAL.saturating_sub(filled));
    format!("{filled_part}{empty_part}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bar_none() {
        assert_eq!(reasoning_bar(None), "▱▱▱");
    }

    #[test]
    fn bar_low() {
        assert_eq!(reasoning_bar(Some(ReasoningEffort::Low)), "▰▱▱");
    }

    #[test]
    fn bar_medium() {
        assert_eq!(reasoning_bar(Some(ReasoningEffort::Medium)), "▰▰▱");
    }

    #[test]
    fn bar_high() {
        assert_eq!(reasoning_bar(Some(ReasoningEffort::High)), "▰▰▰");
    }
}
