mod syntax_highlighter;
mod syntect_bridge;

pub use syntax_highlighter::SyntaxHighlighter;

pub(crate) use syntect_bridge::{find_syntax_for_hint, syntax_set, syntect_to_wisp_style};
