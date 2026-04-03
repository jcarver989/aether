use super::component::{Component, Event};
use crate::focus::{FocusOutcome, FocusRing};
use crate::rendering::columns::{SplitLayout, SplitWidths, side_by_side, side_by_side_with_separator};
use crate::rendering::frame::{Cursor, Frame};
use crate::rendering::render_context::ViewContext;
use crate::style::Style;
use crossterm::event::KeyCode;

pub enum Either<L, R> {
    Left(L),
    Right(R),
}

pub struct SplitPanel<L: Component, R: Component> {
    pub left: L,
    pub right: R,
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

    pub fn focus_left(&mut self) {
        self.focus.focus(0);
    }

    pub fn focus_right(&mut self) {
        self.focus.focus(1);
    }

    pub fn is_left_focused(&self) -> bool {
        self.focus.is_focused(0)
    }

    pub fn widths(&self, total_width: usize) -> SplitWidths {
        self.layout.widths(total_width, self.delta)
    }

    pub fn widen(&mut self) {
        self.delta += self.layout.step();
    }

    pub fn narrow(&mut self) {
        self.delta -= self.layout.step();
    }

    pub fn delta(&self) -> i16 {
        self.delta
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

    #[allow(clippy::cast_possible_truncation)]
    fn render(&mut self, ctx: &ViewContext) -> Frame {
        let widths = self.widths(ctx.size.width as usize);

        let left_ctx = ctx.with_size((widths.left as u16, ctx.size.height));
        let right_ctx = ctx.with_size((widths.right as u16, ctx.size.height));

        let left_frame = self.left.render(&left_ctx);
        let right_frame = self.right.render(&right_ctx);

        let (left_lines, left_cursor) = left_frame.into_parts();
        let (right_lines, right_cursor) = right_frame.into_parts();

        let sep_width = self.separator.as_ref().map_or(0, |(t, _)| t.len());
        let merged = match &self.separator {
            Some((text, style)) => side_by_side_with_separator(&left_lines, &right_lines, widths.left, text, *style),
            None => side_by_side(&left_lines, &right_lines, widths.left),
        };

        let cursor = if self.focus.is_focused(0) && left_cursor.is_visible {
            left_cursor
        } else if self.focus.is_focused(1) && right_cursor.is_visible {
            Cursor::visible(right_cursor.row, right_cursor.col + widths.left + sep_width)
        } else {
            Cursor::hidden()
        };

        Frame::new(merged).with_cursor(cursor)
    }
}
