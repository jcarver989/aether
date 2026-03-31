use super::line::Line;

pub fn side_by_side(left: &[Line], right: &[Line], left_width: usize) -> Vec<Line> {
    let max_rows = left.len().max(right.len());
    let mut result = Vec::with_capacity(max_rows);

    for i in 0..max_rows {
        let mut line = match left.get(i) {
            Some(l) => {
                let mut l = l.clone();
                l.extend_bg_to_width(left_width);
                l
            }
            None => Line::new(" ".repeat(left_width)),
        };

        if let Some(r) = right.get(i) {
            line.append_line(r);
        }

        result.push(line);
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equal_length_columns() {
        let left = vec![Line::new("aa"), Line::new("bb")];
        let right = vec![Line::new("xx"), Line::new("yy")];
        let merged = side_by_side(&left, &right, 4);
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0].plain_text(), "aa  xx");
        assert_eq!(merged[1].plain_text(), "bb  yy");
    }

    #[test]
    fn left_longer_than_right() {
        let left = vec![Line::new("a"), Line::new("b"), Line::new("c")];
        let right = vec![Line::new("x")];
        let merged = side_by_side(&left, &right, 3);
        assert_eq!(merged.len(), 3);
        assert_eq!(merged[0].plain_text(), "a  x");
        assert_eq!(merged[1].plain_text(), "b  ");
        assert_eq!(merged[2].plain_text(), "c  ");
    }

    #[test]
    fn right_longer_than_left() {
        let left = vec![Line::new("a")];
        let right = vec![Line::new("x"), Line::new("y")];
        let merged = side_by_side(&left, &right, 3);
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0].plain_text(), "a  x");
        assert_eq!(merged[1].plain_text(), "   y");
    }

    #[test]
    fn empty_inputs() {
        let merged = side_by_side(&[], &[], 5);
        assert!(merged.is_empty());
    }

    #[test]
    fn preserves_styles() {
        use crate::rendering::style::Style;
        use crossterm::style::Color;

        let left = vec![Line::with_style("hi", Style::fg(Color::Red))];
        let right = vec![Line::with_style("there", Style::fg(Color::Blue))];
        let merged = side_by_side(&left, &right, 6);
        assert_eq!(merged[0].plain_text(), "hi    there");
        assert!(merged[0].spans().len() >= 2);
    }
}
