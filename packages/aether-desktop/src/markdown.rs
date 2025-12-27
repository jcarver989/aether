use dioxus::prelude::*;
use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag, TagEnd};

use crate::syntax::{highlight_code, html_escape};

#[component]
pub fn Markdown(content: String, is_streaming: bool) -> Element {
    // Always render with full markdown + syntax highlighting
    // The static LazyLock ensures resources are only loaded once
    let html = render_markdown_with_highlighting(&content, is_streaming);
    rsx! {
        div {
            class: "markdown-body prose prose-invert max-w-none",
            dangerous_inner_html: "{html}"
        }
    }
}

fn render_markdown_with_highlighting(content: &str, is_streaming: bool) -> String {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_TASKLISTS);

    let parser = Parser::new_ext(content, options);

    let mut html = String::new();
    let mut in_code_block = false;
    let mut code_buffer = String::new();
    let mut current_language = String::new();

    for event in parser {
        match event {
            Event::Start(Tag::CodeBlock(kind)) => {
                in_code_block = true;
                current_language = match kind {
                    CodeBlockKind::Fenced(lang) => lang.to_string(),
                    CodeBlockKind::Indented => String::new(),
                };
            }
            Event::End(TagEnd::CodeBlock) => {
                let highlighted = highlight_code(&code_buffer, &current_language);
                let lang_label = if current_language.is_empty() {
                    "".to_string()
                } else {
                    format!(
                        r#"<div class="text-xs text-gray-500 mb-2">{}</div>"#,
                        html_escape(&current_language)
                    )
                };
                html.push_str(&format!(
                    r#"<div class="code-block bg-gray-900 rounded-lg p-4 my-2 overflow-x-auto">{}<pre class="m-0"><code class="language-{}">{}</code></pre></div>"#,
                    lang_label,
                    html_escape(&current_language),
                    highlighted
                ));
                code_buffer.clear();
                in_code_block = false;
            }
            Event::Text(text) if in_code_block => {
                code_buffer.push_str(&text);
            }
            Event::Start(Tag::Paragraph) => {
                html.push_str("<p class=\"mb-2\">");
            }
            Event::End(TagEnd::Paragraph) => {
                html.push_str("</p>");
            }
            Event::Start(Tag::Heading { level, .. }) => {
                let size = match level {
                    pulldown_cmark::HeadingLevel::H1 => "text-2xl font-bold mb-3",
                    pulldown_cmark::HeadingLevel::H2 => "text-xl font-bold mb-2",
                    pulldown_cmark::HeadingLevel::H3 => "text-lg font-semibold mb-2",
                    _ => "text-base font-semibold mb-1",
                };
                html.push_str(&format!("<h{} class=\"{}\">", level as u8, size));
            }
            Event::End(TagEnd::Heading(level)) => {
                html.push_str(&format!("</h{}>", level as u8));
            }
            Event::Start(Tag::List(None)) => {
                html.push_str("<ul class=\"list-disc list-inside mb-2 ml-4\">");
            }
            Event::End(TagEnd::List(false)) => {
                html.push_str("</ul>");
            }
            Event::Start(Tag::List(Some(_))) => {
                html.push_str("<ol class=\"list-decimal list-inside mb-2 ml-4\">");
            }
            Event::End(TagEnd::List(true)) => {
                html.push_str("</ol>");
            }
            Event::Start(Tag::Item) => {
                html.push_str("<li class=\"mb-1\">");
            }
            Event::End(TagEnd::Item) => {
                html.push_str("</li>");
            }
            Event::Start(Tag::Strong) => {
                html.push_str("<strong class=\"font-bold\">");
            }
            Event::End(TagEnd::Strong) => {
                html.push_str("</strong>");
            }
            Event::Start(Tag::Emphasis) => {
                html.push_str("<em class=\"italic\">");
            }
            Event::End(TagEnd::Emphasis) => {
                html.push_str("</em>");
            }
            Event::Code(code) => {
                html.push_str(&format!(
                    "<code class=\"bg-gray-800 px-1.5 py-0.5 rounded text-sm font-mono text-pink-400\">{}</code>",
                    html_escape(&code)
                ));
            }
            Event::Start(Tag::Link { dest_url, .. }) => {
                html.push_str(&format!(
                    "<a href=\"{}\" class=\"text-blue-400 hover:underline\" target=\"_blank\">",
                    html_escape(&dest_url)
                ));
            }
            Event::End(TagEnd::Link) => {
                html.push_str("</a>");
            }
            Event::Start(Tag::BlockQuote(_)) => {
                html.push_str(
                    "<blockquote class=\"border-l-4 border-gray-600 pl-4 italic text-gray-400 my-2\">",
                );
            }
            Event::End(TagEnd::BlockQuote(_)) => {
                html.push_str("</blockquote>");
            }
            Event::SoftBreak => {
                html.push(' ');
            }
            Event::HardBreak => {
                html.push_str("<br />");
            }
            Event::Text(text) => {
                html.push_str(&html_escape(&text));
            }
            _ => {}
        }
    }

    // Handle unclosed code block (streaming case)
    if in_code_block {
        let highlighted = highlight_code(&code_buffer, &current_language);
        let lang_label = if current_language.is_empty() {
            "".to_string()
        } else {
            format!(
                r#"<div class="text-xs text-gray-500 mb-2">{}</div>"#,
                html_escape(&current_language)
            )
        };
        // Add a blinking cursor indicator if streaming
        let cursor = if is_streaming {
            "<span class=\"animate-pulse\">|</span>"
        } else {
            ""
        };
        html.push_str(&format!(
            r#"<div class="code-block bg-gray-900 rounded-lg p-4 my-2 overflow-x-auto">{}<pre class="m-0"><code class="language-{}">{}{}</code></pre></div>"#,
            lang_label,
            html_escape(&current_language),
            highlighted,
            cursor
        ));
    }

    html
}
