Fetches content from a URL and converts HTML to clean, readable Markdown.

Use this tool when you need to:
- Read documentation from websites
- Research information from online sources
- Access web APIs that return HTML
- Gather information to answer user questions

Usage:
- Provide a valid HTTP/HTTPS URL
- Optionally include a prompt describing what information you're looking for
- HTTP URLs are automatically upgraded to HTTPS
- Large responses are automatically truncated to prevent context overflow
- Default timeout is 30 seconds (configurable up to 60 seconds)

The tool returns:
- Markdown-formatted content (much more token-efficient than raw HTML)
- The final URL after any redirects
- HTTP status code
- Page title if available
- Whether the content was truncated

Best practices:
- Use specific URLs rather than relying on search
- Handle 4xx/5xx status codes gracefully in your logic

Limitations:
- Does not execute JavaScript (cannot handle SPAs or dynamic content)
- No cookie/session handling for authenticated pages
