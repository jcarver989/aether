use iocraft::prelude::Color;

pub fn secondary() -> Color {
    Color::Rgb {
        r: 138,
        g: 43,
        b: 226,
    } // Electric Violet #8A2BE2
}

pub fn primary() -> Color {
    Color::Rgb {
        r: 0x55,
        g: 0xc9,
        b: 0xd3,
    } // #55c9d3
}

pub fn accent() -> Color {
    Color::Rgb {
        r: 255,
        g: 215,
        b: 0,
    } // Gold #FFD700
}

pub fn text_primary() -> Color {
    Color::Rgb {
        r: 255,
        g: 255,
        b: 255,
    } // White #FFFFFF
}

pub fn text_secondary() -> Color {
    Color::Rgb {
        r: 176,
        g: 176,
        b: 208,
    } // Light Gray #B0B0D0
}

pub fn success() -> Color {
    Color::Rgb {
        r: 0,
        g: 255,
        b: 127,
    } // Spring Green #00FF7F
}

pub fn warning() -> Color {
    Color::Rgb {
        r: 255,
        g: 165,
        b: 0,
    } // Orange #FFA500
}

pub fn error() -> Color {
    Color::Rgb {
        r: 255,
        g: 59,
        b: 48,
    } // Red #FF3B30
}

pub fn info() -> Color {
    Color::Rgb {
        r: 78,
        g: 205,
        b: 196,
    } // Turquoise #4ECDC4
}
