use iocraft::prelude::*;

#[component]
pub fn Logo() -> impl Into<AnyElement<'static>> {
    let logo_content = include_str!("logo.txt");

    #[rustfmt::skip]
        let wisp_lines = [
            "██╗    ██╗██╗███████╗██████╗ ",
            "██║    ██║██║██╔════╝██╔══██╗",
            "██║ █╗ ██║██║███████╗██████╔╝",
            "██║███╗██║██║╚════██║██╔═══╝ ",
            "╚███╔███╔╝██║███████║██║     ",
            " ╚══╝╚══╝ ╚═╝╚══════╝╚═╝     ",
        ];

    let lines: Vec<_> = wisp_lines
        .iter()
        .enumerate()
        .map(|(line_idx, line)| {
            let result: String = line
                .chars()
                .map(|ch| {
                    if ch == '█' {
                        // Create vertical lighting gradient: top=full, bottom=light
                        let opacity_char = match line_idx {
                            0 => '█', // Top line - full block (brightest)
                            1 => '▓', // Second line - dark shade
                            2 => '▒', // Third line - medium shade
                            3 => '▒', // Fourth line - medium shade
                            4 => '░', // Fifth line - light shade
                            _ => '░', // Bottom line - light shade (darkest)
                        };
                        opacity_char.clone().to_string()
                    } else {
                        ch.clone().to_string()
                    }
                })
                .collect();

            result
        })
        .collect();

    element! {
        View(flex_direction: FlexDirection::Column) {
            Text(content: logo_content)
            #(
                lines.iter().map(|l| {
                    element! {
                        Text(content: l)
                    }
                })
            )
        }
    }
}
