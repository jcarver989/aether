#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Size {
    pub width: u16,
    pub height: u16,
}

impl From<(u16, u16)> for Size {
    fn from((width, height): (u16, u16)) -> Self {
        Self { width, height }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_tuple() {
        let size: Size = (80, 24).into();
        assert_eq!(size.width, 80);
        assert_eq!(size.height, 24);
    }
}
