use super::component::{Component, Event};
use crate::focus::{FocusOutcome, FocusRing};
use crate::rendering::frame::{Cursor, Frame};
use crate::rendering::line::Line;
use crate::rendering::render_context::ViewContext;
use crate::rendering::soft_wrap::{soft_wrap_lines_with_map, truncate_line};
use crate::style::Style;
use crossterm::event::KeyCode;

pub struct SplitWidths {
    pub left: u16,
    pub right: u16,
}

pub struct SplitLayout {
    base: SplitBase,
    step: i16,
    min_left: usize,
}

enum SplitBase {
    Fraction { numer: usize, denom: usize, min: usize, max: usize },
    Fixed(usize),
}

impl SplitLayout {
    pub fn fraction(numer: usize, denom: usize, min: usize, max: usize) -> Self {
        Self { base: SplitBase::Fraction { numer, denom, min, max }, step: 4, min_left: 12 }
    }

    pub fn fixed(width: usize) -> Self {
        Self { base: SplitBase::Fixed(width), step: 4, min_left: 12 }
    }

    pub fn with_step(mut self, step: i16) -> Self {
        self.step = step;
        self
    }

    pub fn with_min_left(mut self, min: usize) -> Self {
        self.min_left = min;
        self
    }

    fn widths(&self, total_width: u16, delta: i16) -> SplitWidths {
        let total = total_width as usize;
        let base = match self.base {
            SplitBase::Fraction { numer, denom, min, max } => (total * numer / denom).clamp(min, max),
            SplitBase::Fixed(w) => w,
        };
        let left = base.saturating_add_signed(delta.into()).clamp(self.min_left, total / 2);
        let right = total.saturating_sub(left + 1);
        SplitWidths {
            left: u16::try_from(left).unwrap_or(u16::MAX),
            right: u16::try_from(right).unwrap_or(u16::MAX),
        }
    }

    fn step(&self) -> i16 {
        self.step
    }
}

pub enum Either<L, R> {
    Left(L),
    Right(R),
}

pub struct SplitPanel<L: Component, R: Component> {
    left: L,
    right: R,
    layout: SplitLayout,
    delta: i16,
    focus: FocusRing,
    separator: Option<(String, Style)>,
    resize_keys: bool,
}

impl<L: Component, R: Component> SplitPanel<L, R> {
    pub fn new(left: L, right: R, layout: SplitLayout) -> Self {
        Self {
            left,
            right,
            layout,
            delta: 0,
            focus: FocusRing::new(2).without_wrap(),
            separator: None,
            resize_keys: false,
        }
    }

    pub fn with_separator(mut self, text: impl Into<String>, style: Style) -> Self {
        self.separator = Some((text.into(), style));
        self
    }

    pub fn with_resize_keys(mut self) -> Self {
        self.resize_keys = true;
        self
    }

    pub fn left(&self) -> &L {
        &self.left
    }

    pub fn left_mut(&mut self) -> &mut L {
        &mut self.left
    }

    pub fn right(&self) -> &R {
        &self.right
    }

    pub fn right_mut(&mut self) -> &mut R {
        &mut self.right
    }

    pub fn focus_left(&mut self) {
        self.focus.focus(0);
    }

    pub fn focus_right(&mut self) {
        self.focus.focus(1);
    }

    pub fn is_left_focused(&self) -> bool {
        self.focus.is_focused(0)
    }

    pub fn widths(&self, total_width: u16) -> SplitWidths {
        self.layout.widths(total_width, self.delta)
    }

    fn widen(&mut self) {
        self.delta += self.layout.step();
    }

    fn narrow(&mut self) {
        self.delta -= self.layout.step();
    }

    pub fn set_separator_style(&mut self, style: Style) {
        if let Some((_, s)) = &mut self.separator {
            *s = style;
        }
    }
}

impl<L: Component, R: Component> Component for SplitPanel<L, R> {
    type Message = Either<L::Message, R::Message>;

    async fn on_event(&mut self, event: &Event) -> Option<Vec<Self::Message>> {
        if let Event::Key(key) = event {
            if self.resize_keys {
                match key.code {
                    KeyCode::Char('<') => {
                        self.narrow();
                        return Some(vec![]);
                    }
                    KeyCode::Char('>') => {
                        self.widen();
                        return Some(vec![]);
                    }
                    _ => {}
                }
            }

            let outcome = self.focus.handle_key(*key);
            if matches!(outcome, FocusOutcome::FocusChanged) {
                return Some(vec![]);
            }
        }

        if self.focus.is_focused(0) {
            self.left.on_event(event).await.map(|msgs| msgs.into_iter().map(Either::Left).collect())
        } else {
            self.right.on_event(event).await.map(|msgs| msgs.into_iter().map(Either::Right).collect())
        }
    }

    fn render(&mut self, ctx: &ViewContext) -> Frame {
        let widths = self.widths(ctx.size.width);

        let left_ctx = ctx.with_size((widths.left, ctx.size.height));
        let right_ctx = ctx.with_size((widths.right, ctx.size.height));

        let (left_lines, left_cursor) = self.left.render(&left_ctx).into_parts();
        let (right_lines, right_cursor) = self.right.render(&right_ctx).into_parts();

        let (mut right_visual_lines, right_row_starts) = if widths.right == 0 {
            (Vec::new(), vec![0; right_lines.len()])
        } else {
            soft_wrap_lines_with_map(&right_lines, widths.right)
        };

        for line in &mut right_visual_lines {
            line.extend_bg_to_width(widths.right.into());
        }

        let max_rows = usize::from(ctx.size.height);
        let sep_width = self.separator.as_ref().map_or(0, |(t, _)| t.len());
        let mut merged = Vec::with_capacity(max_rows);

        for i in 0..max_rows {
            let mut line = match left_lines.get(i) {
                Some(l) => {
                    let mut l = truncate_line(l, widths.left.into());
                    l.extend_bg_to_width(widths.left.into());
                    l
                }
                None => Line::new(" ".repeat(widths.left.into())),
            };
            if let Some((text, style)) = &self.separator {
                line.push_with_style(text, *style);
            }
            if let Some(right) = right_visual_lines.get(i) {
                line.append_line(right);
            }
            merged.push(line);
        }

        let cursor = if self.focus.is_focused(0) && left_cursor.is_visible {
            if left_cursor.row < max_rows { left_cursor } else { Cursor::hidden() }
        } else if self.focus.is_focused(1) && right_cursor.is_visible && widths.right > 0 {
            let mut row = right_row_starts
                .get(right_cursor.row)
                .copied()
                .unwrap_or_else(|| right_visual_lines.len().saturating_sub(1));
            let mut col = right_cursor.col;
            let right_width = usize::from(widths.right);

            row += col / right_width;
            col %= right_width;

            if row < max_rows {
                Cursor::visible(row, col + usize::from(widths.left) + sep_width)
            } else {
                Cursor::hidden()
            }
        } else {
            Cursor::hidden()
        };

        Frame::new(merged).with_cursor(cursor)
    }
}
