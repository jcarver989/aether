use super::context_bar::slot_bar;
use tui::{Color, Theme};
use utils::ReasoningEffort;

/// Renders a compact labeled 3-slot reasoning effort bar.
///
/// Visual mapping:
/// - `None` => `reasoning [···]` (all empty)
/// - `Low` => `reasoning [■··]` (1 filled)
/// - `Medium` => `reasoning [■■·]` (2 filled)
/// - `High` => `reasoning [■■■]` (3 filled)
pub(crate) fn reasoning_bar(effort: Option<ReasoningEffort>) -> String {
    let filled = match effort {
        None => 0,
        Some(ReasoningEffort::Low) => 1,
        Some(ReasoningEffort::Medium) => 2,
        Some(ReasoningEffort::High) => 3,
    };
    format!("reasoning {}", slot_bar(filled, 3))
}

/// Returns the appropriate theme color for the given reasoning effort.
///
/// - None/Low  → `text_secondary` (subdued)
/// - Medium    → `info`
/// - High      → `success`
pub(crate) fn reasoning_color(effort: Option<ReasoningEffort>, theme: &Theme) -> Color {
    match effort {
        None | Some(ReasoningEffort::Low) => theme.text_secondary(),
        Some(ReasoningEffort::Medium) => theme.info(),
        Some(ReasoningEffort::High) => theme.success(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bar_none() {
        assert_eq!(reasoning_bar(None), "reasoning [···]");
    }

    #[test]
    fn bar_low() {
        assert_eq!(reasoning_bar(Some(ReasoningEffort::Low)), "reasoning [■··]");
    }

    #[test]
    fn bar_medium() {
        assert_eq!(
            reasoning_bar(Some(ReasoningEffort::Medium)),
            "reasoning [■■·]"
        );
    }

    #[test]
    fn bar_high() {
        assert_eq!(
            reasoning_bar(Some(ReasoningEffort::High)),
            "reasoning [■■■]"
        );
    }

    #[test]
    fn color_tiers() {
        let theme = Theme::default();
        assert_eq!(reasoning_color(None, &theme), theme.text_secondary());
        assert_eq!(
            reasoning_color(Some(ReasoningEffort::Low), &theme),
            theme.text_secondary()
        );
        assert_eq!(
            reasoning_color(Some(ReasoningEffort::Medium), &theme),
            theme.info()
        );
        assert_eq!(
            reasoning_color(Some(ReasoningEffort::High), &theme),
            theme.success()
        );
    }
}
