use crate::Component;
use crate::component::RenderContext;
use crate::line::Line;

pub const BRAILLE_FRAMES: &[char] = &['⠒', '⠮', '⠷', '⢷', '⡾', '⣯', '⣽', '⣿', '⣭', '⢯'];

pub struct Spinner {
    tick: u16,
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

    pub fn reset(&mut self) {
        self.tick = 0;
        self.visible = true;
    }

    #[allow(dead_code)]
    pub fn set_tick(&mut self, tick: u16) {
        self.tick = tick;
    }

    /// Advance the animation state. Call this on tick events.
    pub fn on_tick(&mut self) {
        if self.visible {
            self.tick = self.tick.wrapping_add(1);
        }
    }
}

impl Default for Spinner {
    fn default() -> Self {
        Self::braille()
    }
}

impl Component for Spinner {
    fn render(&self, context: &RenderContext) -> Vec<Line> {
        if !self.visible {
            return vec![];
        }

        let frame = self.current_frame();
        let mut line = Line::default();
        line.push_styled(frame.to_string(), context.theme.info());
        vec![line]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invisible_renders_empty() {
        let spinner = Spinner::default();
        let ctx = RenderContext::new((80, 24));
        let lines = spinner.render(&ctx);
        assert!(lines.is_empty());
    }

    #[test]
    fn visible_renders_one_line() {
        let mut spinner = Spinner::default();
        spinner.visible = true;
        let ctx = RenderContext::new((80, 24));
        let lines = spinner.render(&ctx);
        assert_eq!(lines.len(), 1);
    }

    #[test]
    fn different_ticks_produce_different_output() {
        let ctx = RenderContext::new((80, 24));

        let mut spinner_a = Spinner::default();
        spinner_a.visible = true;

        let mut spinner_b = Spinner::default();
        spinner_b.visible = true;
        spinner_b.set_tick(1);

        let a = spinner_a.render(&ctx)[0].plain_text();
        let b = spinner_b.render(&ctx)[0].plain_text();

        assert_ne!(a, b);
    }

    #[test]
    fn cycles_after_full_rotation() {
        let ctx = RenderContext::new((80, 24));

        let mut spinner_a = Spinner::default();
        spinner_a.visible = true;

        let mut spinner_b = Spinner::default();
        spinner_b.visible = true;
        #[allow(clippy::cast_possible_truncation)]
        spinner_b.set_tick(BRAILLE_FRAMES.len() as u16);

        let a = spinner_a.render(&ctx)[0].plain_text();
        let b = spinner_b.render(&ctx)[0].plain_text();

        assert_eq!(a, b);
    }

    #[test]
    fn custom_frames() {
        static CUSTOM: &[char] = &['|', '/', '-', '\\'];
        let mut spinner = Spinner::new(CUSTOM);
        spinner.set_tick(1);
        spinner.visible = true;
        assert_eq!(spinner.current_frame(), '/');
    }

    #[test]
    fn on_tick_advances_when_visible() {
        let mut spinner = Spinner::default();
        spinner.visible = true;
        spinner.on_tick();
        assert_eq!(spinner.current_frame(), BRAILLE_FRAMES[1]);
    }

    #[test]
    fn on_tick_noop_when_invisible() {
        let mut spinner = Spinner::default();
        spinner.on_tick();
        assert_eq!(spinner.current_frame(), BRAILLE_FRAMES[0]);
    }

    #[test]
    fn reset_sets_tick_zero_and_visible() {
        let mut spinner = Spinner::default();
        spinner.set_tick(5);
        spinner.visible = false;
        spinner.reset();
        assert!(spinner.visible);
        assert_eq!(spinner.current_frame(), BRAILLE_FRAMES[0]);
    }
}
