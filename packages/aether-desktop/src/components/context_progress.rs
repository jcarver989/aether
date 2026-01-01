//! Context usage progress bar component.
//!
//! Displays a linear progress bar showing context window usage.

use dioxus::prelude::*;

/// A linear progress bar for context usage.
///
/// Changes color based on usage level:
/// - Green (< 50%): Safe zone
/// - Yellow (50-70%): Warning zone
/// - Red (>= 70%): Critical zone
#[component]
pub fn ContextProgressBar(
    /// Usage ratio from 0.0 to 1.0
    usage: f64,
) -> Element {
    let usage_clamped = usage.clamp(0.0, 1.0);
    let percentage = (usage_clamped * 100.0) as u32;
    let width_percent = format!("{}%", percentage);

    let bar_color = usage_color(usage_clamped);

    rsx! {
        div {
            class: "flex items-center gap-2 w-full",

            // Progress bar container
            div {
                class: "flex-1 bg-gray-700 rounded-full h-1.5 overflow-hidden",

                // Progress fill
                div {
                    class: "h-1.5 rounded-full transition-all duration-300",
                    style: "width: {width_percent}; background-color: {bar_color};",
                }
            }

            // Percentage text
            span {
                class: "text-xs text-gray-400 w-8 text-right",
                "{percentage}%"
            }
        }
    }
}

/// Returns the CSS color value based on usage level.
///
/// - Green (< 50%): Safe zone
/// - Yellow (50-70%): Warning zone
/// - Red (>= 70%): Critical zone
fn usage_color(usage: f64) -> &'static str {
    if usage < 0.5 {
        "#22c55e" // green-500
    } else if usage < 0.7 {
        "#eab308" // yellow-500
    } else {
        "#ef4444" // red-500
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_usage_color_green_zone() {
        assert_eq!(usage_color(0.0), "#22c55e");
        assert_eq!(usage_color(0.3), "#22c55e");
        assert_eq!(usage_color(0.49), "#22c55e");
    }

    #[test]
    fn test_usage_color_yellow_zone() {
        assert_eq!(usage_color(0.5), "#eab308");
        assert_eq!(usage_color(0.6), "#eab308");
        assert_eq!(usage_color(0.69), "#eab308");
    }

    #[test]
    fn test_usage_color_red_zone() {
        assert_eq!(usage_color(0.7), "#ef4444");
        assert_eq!(usage_color(0.85), "#ef4444");
        assert_eq!(usage_color(1.0), "#ef4444");
    }
}
