use super::component::RenderContext;
use super::screen::Line;
use crate::tui::Component;

pub const BRAILLE_FRAMES: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

pub struct Spinner {
    pub tick: u16,
    pub visible: bool,
    frames: &'static [char],
}

impl Spinner {
    pub fn new(frames: &'static [char]) -> Self {
        Self {
            tick: 0,
            visible: false,
            frames,
        }
    }

    pub fn braille() -> Self {
        Self::new(BRAILLE_FRAMES)
    }

    pub fn current_frame(&self) -> char {
        self.frames[self.tick as usize % self.frames.len()]
    }
}

impl Default for Spinner {
    fn default() -> Self {
        Self::braille()
    }
}

impl Component for Spinner {
    fn render(&mut self, context: &RenderContext) -> Vec<Line> {
        if !self.visible {
            return vec![];
        }

        let frame = self.current_frame();
        let mut line = Line::default();
        line.push_styled(frame.to_string(), context.theme.info);
        vec![line]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invisible_renders_empty() {
        let mut spinner = Spinner {
            tick: 0,
            visible: false,
            ..Spinner::default()
        };
        let ctx = RenderContext::new((80, 24));
        let lines = spinner.render(&ctx);
        assert!(lines.is_empty());
    }

    #[test]
    fn visible_renders_one_line() {
        let mut spinner = Spinner {
            tick: 0,
            visible: true,
            ..Spinner::default()
        };
        let ctx = RenderContext::new((80, 24));
        let lines = spinner.render(&ctx);
        assert_eq!(lines.len(), 1);
    }

    #[test]
    fn different_ticks_produce_different_output() {
        let ctx = RenderContext::new((80, 24));

        let mut spinner_a = Spinner {
            tick: 0,
            visible: true,
            ..Spinner::default()
        };
        let mut spinner_b = Spinner {
            tick: 1,
            visible: true,
            ..Spinner::default()
        };

        let a = spinner_a.render(&ctx)[0].plain_text();
        let b = spinner_b.render(&ctx)[0].plain_text();

        assert_ne!(a, b);
    }

    #[test]
    fn cycles_after_full_rotation() {
        let ctx = RenderContext::new((80, 24));

        let mut spinner_a = Spinner {
            tick: 0,
            visible: true,
            ..Spinner::default()
        };
        let mut spinner_b = Spinner {
            #[allow(clippy::cast_possible_truncation)]
            tick: BRAILLE_FRAMES.len() as u16,
            visible: true,
            ..Spinner::default()
        };

        let a = spinner_a.render(&ctx)[0].plain_text();
        let b = spinner_b.render(&ctx)[0].plain_text();

        assert_eq!(a, b);
    }

    #[test]
    fn current_frame_returns_correct_char() {
        let spinner = Spinner {
            tick: 2,
            visible: true,
            ..Spinner::default()
        };
        assert_eq!(spinner.current_frame(), '⠹');
    }

    #[test]
    fn custom_frames() {
        static CUSTOM: &[char] = &['|', '/', '-', '\\'];
        let spinner = Spinner {
            tick: 1,
            visible: true,
            frames: CUSTOM,
        };
        assert_eq!(spinner.current_frame(), '/');
    }
}
