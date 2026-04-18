mod headings;
mod renderer;
mod source_map;
mod table;

use pulldown_cmark::Options;

pub use headings::{MarkdownHeading, parse_markdown_headings};
pub use renderer::{MarkdownBlock, MarkdownRenderResult, SourceMappedLine, render_markdown_result};

pub(super) fn pulldown_options() -> Options {
    Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TABLES
}
