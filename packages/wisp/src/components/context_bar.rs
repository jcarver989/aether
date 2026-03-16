use tui::{Color, Theme};

/// Renders a bracketed slot bar with `filled` of `total` slots filled.
///
/// Example: `slot_bar(2, 3)` => `[■■·]`
pub(crate) fn slot_bar(filled: usize, total: usize) -> String {
    let slots: String = (0..total)
        .map(|i| if i < filled { '■' } else { '·' })
        .collect();
    format!("[{slots}]")
}

/// Renders a compact 5-slot context gauge with label and percentage.
///
/// Each slot represents 20% of context. Visual mapping:
/// - 100% => `ctx [■■■■■] 100%`
/// - 50%  => `ctx [■■···] 50%`
/// - 10%  => `ctx [·····] 10%`
pub(crate) fn context_bar(pct: u8) -> String {
    const TOTAL: usize = 5;
    let filled = ((pct as usize) * TOTAL + 50) / 100;
    format!("ctx {} {pct}%", slot_bar(filled, TOTAL))
}

/// Returns the appropriate theme color for the given context percentage.
///
/// Tiers:
/// - >30%  → text_secondary (subtle awareness)
/// - 15-30% → warning (yellow)
/// - <15%  → error (red, attention needed)
pub(crate) fn context_color(pct: u8, theme: &Theme) -> Color {
    match pct {
        0..15 => theme.error(),
        15..30 => theme.warning(),
        _ => theme.text_secondary(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slot_bar_empty() {
        assert_eq!(slot_bar(0, 3), "[···]");
    }

    #[test]
    fn slot_bar_partial() {
        assert_eq!(slot_bar(2, 3), "[■■·]");
    }

    #[test]
    fn slot_bar_full() {
        assert_eq!(slot_bar(3, 3), "[■■■]");
    }

    #[test]
    fn bar_full() {
        assert_eq!(context_bar(100), "ctx [■■■■■] 100%");
    }

    #[test]
    fn bar_empty() {
        assert_eq!(context_bar(0), "ctx [·····] 0%");
    }

    #[test]
    fn bar_half() {
        assert_eq!(context_bar(50), "ctx [■■■··] 50%");
    }

    #[test]
    fn bar_low() {
        assert_eq!(context_bar(10), "ctx [■····] 10%");
    }

    #[test]
    fn bar_high() {
        assert_eq!(context_bar(82), "ctx [■■■■·] 82%");
    }

    #[test]
    fn bar_rounding() {
        // 20% = exactly 1 slot
        assert_eq!(context_bar(20), "ctx [■····] 20%");
        // 60% = 3 slots
        assert_eq!(context_bar(60), "ctx [■■■··] 60%");
    }

    #[test]
    fn color_tiers() {
        let theme = Theme::default();
        assert_eq!(context_color(10, &theme), theme.error());
        assert_eq!(context_color(14, &theme), theme.error());
        assert_eq!(context_color(15, &theme), theme.warning());
        assert_eq!(context_color(29, &theme), theme.warning());
        assert_eq!(context_color(30, &theme), theme.text_secondary());
        assert_eq!(context_color(60, &theme), theme.text_secondary());
        assert_eq!(context_color(61, &theme), theme.text_secondary());
        assert_eq!(context_color(100, &theme), theme.text_secondary());
    }
}
